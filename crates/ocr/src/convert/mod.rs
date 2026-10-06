//! Converts a whole chapter PDF into its folder of pages: cut the pages out, convert them a few
//! at a time, and write `chapter.json` last. A finished chapter is never converted again.

mod checks;
mod figure;
mod page;
mod poppler;
pub mod reply;
mod route;
mod save;
pub mod services;
mod summary;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use futures_util::StreamExt;
use futures_util::stream;
use sha2::{Digest, Sha256};

use crate::content::{
    ChapterIndex, ContentError, FORMAT_VERSION, PAGE_IMAGE_FILE, PAGE_PDF_FILE, PageIndex,
    TEXT_LAYER_FILE, book_folder_name, chapter_folder_name, is_partial_page_folder_name,
    page_folder_name, parse_chapter_file_name, partial_page_folder_name,
};

use page::convert_page;
use services::{ClaudeError, LiveServices, PageServices, PageSource, ServiceError};

pub use checks::{PieceRef, ReplyFault};
pub use page::PageError;
pub use poppler::PopplerError;
pub use summary::{
    CallTally, ConversionSummary, OutOfSequence, PageToCheck, PieceCounts, RouteCounts,
};

const PARALLEL_PAGES: usize = 4;
const MODEL_IMAGE_FILE: &str = "model-view.png";

#[derive(thiserror::Error, Debug)]
pub enum ConvertError {
    #[error(transparent)]
    Content(#[from] ContentError),

    #[error("{}", ClaudeError::ApiKeySet)]
    ApiKeySet,

    #[error("could not read the chapter pdf {}", path.display())]
    SourceUnreadable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error(
        "{} already holds a different pdf for this chapter (it was made from {saved_file}, and {given_file} has other contents); nothing was changed, so remove that folder to convert again",
        folder.display()
    )]
    DifferentSource {
        folder: PathBuf,
        saved_file: String,
        given_file: String,
    },

    #[error(transparent)]
    Poppler(#[from] PopplerError),

    #[error("the outside services could not be started")]
    Services(#[from] ServiceError),

    #[error("page {position} could not be converted (its working folder is {})", folder.display())]
    PageFailed {
        position: u32,
        folder: PathBuf,
        #[source]
        source: Box<PageError>,
    },
}

/// One chapter PDF and where its pages are saved.
#[derive(Debug, Clone)]
pub struct ChapterJob {
    book_title: String,
    chapter_pdf: PathBuf,
    source_file_name: String,
    chapter_number: u32,
    chapter_name: String,
    chapter_folder: PathBuf,
}

impl ChapterJob {
    /// Reads the chapter number and name from the file's name, and works out the chapter's
    /// folder under `output_root`. Opens no file.
    ///
    /// # Errors
    /// Fails if the name is not `chapter-<number>-<name>.pdf` or the book title has no letters
    /// or digits.
    pub fn new(
        book_title: &str,
        chapter_pdf: &Path,
        output_root: &Path,
    ) -> Result<Self, ConvertError> {
        let source_file_name = chapter_pdf
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let chapter = parse_chapter_file_name(&source_file_name)?;
        let chapter_folder = output_root
            .join(book_folder_name(book_title)?)
            .join(chapter_folder_name(chapter.number));
        Ok(Self {
            book_title: book_title.to_owned(),
            chapter_pdf: chapter_pdf.to_path_buf(),
            source_file_name,
            chapter_number: chapter.number,
            chapter_name: chapter.name,
            chapter_folder,
        })
    }

    /// `<output root>/<book folder>/chapter-<number>`.
    pub fn chapter_folder(&self) -> PathBuf {
        self.chapter_folder.clone()
    }
}

enum Prepared {
    Finished(ConversionSummary),
    ToDo { source_sha256: String },
}

/// Converts a chapter with the real services: `claude` for Haiku and Sonnet, and Jev.
///
/// A chapter that is already finished is not touched: no service is started and no key is
/// needed. An interrupted chapter resumes at the pages that are missing.
///
/// # Errors
/// - [`ConvertError::ApiKeySet`] if `ANTHROPIC_API_KEY` is set, even when the chapter is
///   finished
/// - [`ConvertError::Services`] if Jev cannot be reached or refuses its key
/// - [`ConvertError::PageFailed`] for the lowest page that failed; finished pages stay saved
pub async fn convert_chapter(job: &ChapterJob) -> Result<ConversionSummary, ConvertError> {
    if services::api_key_is_set() {
        return Err(ConvertError::ApiKeySet);
    }
    match prepare(job)? {
        Prepared::Finished(summary) => Ok(summary),
        Prepared::ToDo { source_sha256 } => {
            let services = LiveServices::from_env().await?;
            run(job, &source_sha256, &services).await
        }
    }
}

/// Like [`convert_chapter`], with the paid calls made by `services`. It does not look at the
/// environment.
///
/// # Errors
/// The same as [`convert_chapter`], except `ApiKeySet` and `Services`, which need the environment.
pub async fn convert_chapter_with<S: PageServices>(
    job: &ChapterJob,
    services: &S,
) -> Result<ConversionSummary, ConvertError> {
    match prepare(job)? {
        Prepared::Finished(summary) => Ok(summary),
        Prepared::ToDo { source_sha256 } => run(job, &source_sha256, services).await,
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Reads the PDF and the chapter folder and decides what is left to do. Writes nothing.
fn prepare(job: &ChapterJob) -> Result<Prepared, ConvertError> {
    let bytes =
        std::fs::read(&job.chapter_pdf).map_err(|source| ConvertError::SourceUnreadable {
            path: job.chapter_pdf.clone(),
            source,
        })?;
    let source_sha256 = sha256_hex(&bytes);
    let saved = match ChapterIndex::read(&job.chapter_folder) {
        Ok(saved) => saved,
        Err(ContentError::Read { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Prepared::ToDo { source_sha256 });
        }
        Err(error) => return Err(error.into()),
    };
    if saved.source_sha256 != source_sha256 {
        return Err(ConvertError::DifferentSource {
            folder: job.chapter_folder.clone(),
            saved_file: saved.source_file,
            given_file: job.source_file_name.clone(),
        });
    }
    // Every page is looked at, so a read error on any of them is returned.
    let mut every_page_saved = true;
    for position in 1..=saved.page_count {
        every_page_saved &= is_saved(&job.chapter_folder, position)?;
    }
    if saved.finished && saved.format_version == FORMAT_VERSION && every_page_saved {
        let summary = ConversionSummary::from_folder(&job.chapter_folder, 0, CallTally::default())?;
        return Ok(Prepared::Finished(summary));
    }
    Ok(Prepared::ToDo { source_sha256 })
}

/// A missing `page.json`, one that is not a page index and one at another format version all
/// mean the page is not saved. Any other read error is returned: it says nothing about the
/// page, and treating it as "not saved" would delete a page that was paid for.
fn is_saved(chapter_folder: &Path, position: u32) -> Result<bool, ContentError> {
    match PageIndex::read(&chapter_folder.join(page_folder_name(position))) {
        Ok(page) => Ok(page.format_version == FORMAT_VERSION),
        Err(ContentError::Read { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
            Ok(false)
        }
        Err(ContentError::Parse { .. }) => Ok(false),
        Err(error) => Err(error),
    }
}

fn write_error(path: &Path) -> impl FnOnce(std::io::Error) -> ContentError + '_ {
    |source| ContentError::Write {
        path: path.to_path_buf(),
        source,
    }
}

async fn run<S: PageServices>(
    job: &ChapterJob,
    source_sha256: &str,
    services: &S,
) -> Result<ConversionSummary, ConvertError> {
    let folder = &job.chapter_folder;
    let page_count = poppler::page_count(&job.chapter_pdf).await?;
    std::fs::create_dir_all(folder).map_err(write_error(folder))?;
    let mut index = ChapterIndex {
        format_version: FORMAT_VERSION,
        book_title: job.book_title.clone(),
        chapter_number: job.chapter_number,
        chapter_name: job.chapter_name.clone(),
        source_file: job.source_file_name.clone(),
        source_sha256: source_sha256.to_owned(),
        page_count,
        finished: false,
    };
    // SMELL: nothing stops two runs on the same chapter at once. They would remove each other's
    // working folders. Run one `ocr` run per chapter.
    index.write(folder)?;

    let to_do = pages_to_do(folder, page_count)?;
    let mut sources = Vec::new();
    for &position in &to_do {
        sources.push(cut_page_out(job, position).await?);
    }

    let calls = convert_pages(services, folder, sources).await?;

    index.finished = true;
    index.write(folder)?;
    Ok(ConversionSummary::from_folder(
        folder,
        to_do.len() as u32,
        calls,
    )?)
}

/// Converts the pages a few at a time and adds up what their calls cost. When pages fail, the
/// error is for the one with the lowest position.
async fn convert_pages<S: PageServices>(
    services: &S,
    chapter_folder: &Path,
    sources: Vec<PageSource>,
) -> Result<CallTally, ConvertError> {
    let failed = AtomicBool::new(false);
    let failed = &failed;
    let results: Vec<(u32, Option<Result<CallTally, PageError>>)> = stream::iter(sources)
        .map(|source| async move {
            // After the first failure no new page starts. Pages already running finish.
            if failed.load(Ordering::Relaxed) {
                return (source.position, None);
            }
            let outcome = convert_page(services, &source, chapter_folder).await;
            if outcome.is_err() {
                failed.store(true, Ordering::Relaxed);
            }
            (source.position, Some(outcome))
        })
        .buffer_unordered(PARALLEL_PAGES)
        .collect()
        .await;

    let mut calls = CallTally::default();
    let mut first_failure: Option<(u32, PageError)> = None;
    for (position, outcome) in results {
        match outcome {
            Some(Ok(page_calls)) => calls.add(&page_calls),
            Some(Err(error))
                if first_failure
                    .as_ref()
                    .is_none_or(|(lowest, _)| position < *lowest) =>
            {
                first_failure = Some((position, error));
            }
            _ => {}
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
