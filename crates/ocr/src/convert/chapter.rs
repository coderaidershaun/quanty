//! The work of a chapter run once pages are known to be left: tidy the folder, cut the missing
//! pages out, convert them a few at a time, and mark the chapter finished last.

use std::path::Path;
use std::pin::pin;
use std::sync::atomic::{AtomicBool, Ordering};

use futures_util::StreamExt;
use futures_util::stream;

use super::page::convert_page;
use super::services::{PageServices, PageSource};
use super::{
    CallTally, ChapterJob, ConversionSummary, ConvertError, PageError, PageProgress, is_saved,
    poppler, write_error,
};
use crate::content::{
    ChapterIndex, ContentError, FORMAT_VERSION, PAGE_IMAGE_FILE, PAGE_PDF_FILE, TEXT_LAYER_FILE,
    is_partial_page_folder_name, page_folder_name, partial_page_folder_name,
};

const PARALLEL_PAGES: usize = 4;
const MODEL_IMAGE_FILE: &str = "model-view.png";

pub(super) async fn run<S: PageServices>(
    job: &ChapterJob,
    source_sha256: &str,
    services: &S,
    mut on_progress: impl FnMut(PageProgress),
) -> Result<ConversionSummary, ConvertError> {
    let folder = &job.chapter_folder;
    let page_count = poppler::page_count(&job.chapter_pdf).await?;
    std::fs::create_dir_all(folder).map_err(write_error(folder))?;
    let mut index = ChapterIndex {
        format_version: FORMAT_VERSION,
        media_title: job.media_title.clone(),
        name: job.name.clone(),
        source_file: job.source_file_name.clone(),
        source_sha256: source_sha256.to_owned(),
        page_count,
        finished: false,
    };
    // SMELL: nothing stops two runs on the same chapter at once. They would remove each other's
    // working folders. Run one `ocr` run per chapter.
    index.write(folder)?;

    let to_do = pages_to_do(folder, page_count)?;
    on_progress(PageProgress::Pages {
        total: page_count,
        done_before: page_count - to_do.len() as u32,
    });
    let mut sources = Vec::new();
    for &position in &to_do {
        sources.push(cut_page_out(job, position).await?);
    }

    let calls = convert_pages(services, folder, sources, on_progress).await?;

    index.finished = true;
    index.write(folder)?;
    Ok(ConversionSummary::from_folder(
        folder,
        to_do.len() as u32,
        calls,
    )?)
}

/// Converts the pages a few at a time and adds up their calls, telling each page as it ends.
/// When pages fail, the error is for the one with the lowest position.
async fn convert_pages<S: PageServices>(
    services: &S,
    chapter_folder: &Path,
    sources: Vec<PageSource>,
    mut on_progress: impl FnMut(PageProgress),
) -> Result<CallTally, ConvertError> {
    let failed = AtomicBool::new(false);
    let failed = &failed;
    let results = stream::iter(sources)
        .map(|source| async move {
            if failed.load(Ordering::Relaxed) {
                return (source.position, None);
            }
            let outcome = convert_page(services, &source, chapter_folder).await;
            if outcome.is_err() {
                failed.store(true, Ordering::Relaxed);
            }
            (source.position, Some(outcome))
        })
        .buffer_unordered(PARALLEL_PAGES);
    let mut results = pin!(results);

    let mut calls = CallTally::default();
    let mut first_failure: Option<(u32, PageError)> = None;
    while let Some((position, outcome)) = results.next().await {
        match outcome {
            Some(Ok(page_calls)) => {
                calls.add(&page_calls);
                on_progress(PageProgress::PageDone {
                    position,
                    calls: page_calls,
                });
            }
            Some(Err(error)) => {
                on_progress(PageProgress::PageFailed { position });
                if first_failure
                    .as_ref()
                    .is_none_or(|(lowest, _)| position < *lowest)
                {
                    first_failure = Some((position, error));
                }
            }
            None => {}
        }
    }
    match first_failure {
        Some((position, error)) => Err(ConvertError::PageFailed {
            position,
            folder: chapter_folder.join(partial_page_folder_name(position)),
            source: Box::new(error),
        }),
        None => Ok(calls),
    }
}

/// Tidies the chapter folder for a resume and returns the pages that still need converting.
/// Working folders left by an interrupted run are removed, and so is any page folder whose
/// `page.json` cannot be read back.
fn pages_to_do(folder: &Path, page_count: u32) -> Result<Vec<u32>, ContentError> {
    let read_error = |source| ContentError::Read {
        path: folder.to_path_buf(),
        source,
    };
    for entry in std::fs::read_dir(folder).map_err(read_error)? {
        let path = entry.map_err(read_error)?.path();
        let is_leftover = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(is_partial_page_folder_name);
        if is_leftover {
            std::fs::remove_dir_all(&path).map_err(write_error(&path))?;
        }
    }
    let mut to_do = Vec::new();
    for position in 1..=page_count {
        if is_saved(folder, position)? {
            continue;
        }
        let page_folder = folder.join(page_folder_name(position));
        if page_folder.exists() {
            std::fs::remove_dir_all(&page_folder).map_err(write_error(&page_folder))?;
        }
        to_do.push(position);
    }
    Ok(to_do)
}

/// Does all the local work for a page in its working folder, so no paid call starts before it
/// is finished.
async fn cut_page_out(job: &ChapterJob, position: u32) -> Result<PageSource, ConvertError> {
    let partial = job.chapter_folder.join(partial_page_folder_name(position));
    std::fs::create_dir_all(&partial).map_err(write_error(&partial))?;
    let pdf = partial.join(PAGE_PDF_FILE);
    poppler::cut_page(&job.chapter_pdf, position, &pdf).await?;
    let text_layer = poppler::text_layer(&pdf).await?;
    let text_layer_file = partial.join(TEXT_LAYER_FILE);
    std::fs::write(&text_layer_file, &text_layer).map_err(write_error(&text_layer_file))?;
    poppler::render_image(&pdf, &partial.join(PAGE_IMAGE_FILE)).await?;
    let image = partial.join(MODEL_IMAGE_FILE);
    poppler::render_model_image(&pdf, &image).await?;
    Ok(PageSource {
        position,
        pdf,
        image,
        text_layer,
    })
}
