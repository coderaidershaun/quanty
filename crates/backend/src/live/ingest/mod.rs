//! Checks a document of a media before it is ingested, and ingests it.

mod preflight;
mod run;

use crate::live::convert::IntoView;

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use ocr::{ChapterJob, MediaDocument};

use crate::live::{LiveContext, Services};
use contract::{Category, DocumentName, Failure, FailureKind, IngestRequest};

pub use preflight::preflight;
pub use run::run;

const PDF_START: &[u8] = b"%PDF-";

/// These checks cost nothing, and a check and a start share them. Nothing is written and no
/// store is asked. The name of the document comes with the request as the person typed it, so
/// the PDF may have any file name. A title typed for a book is a part of the book, which is
/// stored under the book's title.
fn chapter_job<S: Services>(
    cx: &LiveContext<S>,
    ingest: &IngestRequest,
) -> Result<ChapterJob, Failure> {
    let pdf = uploaded_source(cx, ingest)?;
    check_is_a_pdf(&pdf)?;
    chapter_job_at(cx, ingest, &pdf)
}

fn chapter_job_at<S: Services>(
    cx: &LiveContext<S>,
    ingest: &IngestRequest,
    pdf: &Path,
) -> Result<ChapterJob, Failure> {
    let document = match (ingest.category, &ingest.name) {
        (Category::Book, DocumentName::Title(part)) => {
            MediaDocument::book_part(&ingest.media, part)
        }
        _ => MediaDocument {
            media_title: ingest.media.clone(),
            name: (&ingest.name).into_view(),
        },
    };
    ChapterJob::new(document, pdf, &cx.config().content_folder).map_err(|error| cx.failure(error))
}

fn uploaded_source<S: Services>(
    cx: &LiveContext<S>,
    ingest: &IngestRequest,
) -> Result<PathBuf, Failure> {
    rag_ingestion::check_upload_file_name(&ingest.file_name)
        .map_err(|error| Failure::new(FailureKind::BadFile, error.to_string()))?;
    let uploads = cx.home().join(crate::UPLOADS);
    let folder = uploads.join(ingest.pdf.0.to_string());
    let pdf = folder.join(&ingest.file_name);
    if !pdf
        .try_exists()
        .map_err(|error| upload_failure(&pdf, "read", error))?
    {
        let persistent = persistent_path(cx, ingest)?;
        if !inside(&persistent, &cx.config().content_folder.join("_uploads"))? {
            return Err(Failure::new(
                FailureKind::BadFile,
                "the PDF is outside the persistent upload folder",
            ));
        }
        return Ok(persistent);
    }
    if !inside(&folder, &uploads)? || !inside(&pdf, &folder)? {
        return Err(Failure::new(
            FailureKind::BadFile,
            "the PDF is outside its upload folder",
        ));
    }
    Ok(pdf)
}

/// Where the PDF of the request is kept for good, beneath the content folder.
fn persistent_path<S: Services>(
    cx: &LiveContext<S>,
    ingest: &IngestRequest,
) -> Result<PathBuf, Failure> {
    let media =
        ocr::content::media_folder_name(&ingest.media).map_err(|error| cx.failure(error))?;
    rag_ingestion::uploaded_pdf_path(&cx.config().content_folder, &media, &ingest.file_name)
        .map_err(|error| Failure::new(FailureKind::BadFile, error.to_string()))
}

/// Whether `child` is beneath `root` once both are resolved.
fn inside(child: &Path, root: &Path) -> Result<bool, Failure> {
    let resolve = |path: &Path| {
        path.canonicalize()
            .map_err(|error| upload_failure(path, "read", error))
    };
    let root = resolve(root)?;
    Ok(resolve(child)?.starts_with(root))
}

fn prepare_upload<S: Services>(
    cx: &LiveContext<S>,
    ingest: &IngestRequest,
) -> Result<ChapterJob, Failure> {
    let source = uploaded_source(cx, ingest)?;
    check_is_a_pdf(&source)?;
    let saved = persistent_path(cx, ingest)?;
    let job = chapter_job_at(cx, ingest, &saved)?;
    if source == saved {
        return Ok(job);
    }
    let bytes = std::fs::read(&source).map_err(|error| upload_failure(&source, "read", error))?;
    rag_ingestion::save_uploaded_pdf(&saved, &bytes)
        .map_err(|error| upload_failure(&saved, "save", error))?;
    std::fs::remove_file(&source).map_err(|error| upload_failure(&source, "remove", error))?;
    if let Some(folder) = source.parent() {
        std::fs::remove_dir(folder).map_err(|error| upload_failure(folder, "remove", error))?;
    }
    Ok(job)
}

fn upload_failure(path: &Path, action: &str, error: std::io::Error) -> Failure {
    Failure::new(FailureKind::BadFile, format!("could not {action} {}: {error}", path.display()))
        .with_hint("Upload the PDF again. Check that the server can read and write its upload and content folders.")
}

fn file_name_of(pdf: &Path) -> String {
    pdf.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The size is not limited here.
fn check_is_a_pdf(pdf: &Path) -> Result<(), Failure> {
    let name = file_name_of(pdf);
    let refused = |problem: &str, detail: String| {
        let hint = format!("{name} {problem}. Choose a PDF that opens in a PDF reader.");
        Failure::new(FailureKind::BadFile, detail).with_hint(hint)
    };
    let cannot_read = |error: std::io::Error| {
        refused(
            "cannot be read",
            format!("could not read {}: {error}", pdf.display()),
        )
    };
    let file = File::open(pdf).map_err(cannot_read)?;
    if !file.metadata().map_err(cannot_read)?.is_file() {
        let detail = format!("{} is not a file", pdf.display());
        return Err(refused("is not a file", detail));
    }
    let mut start = Vec::new();
    file.take(PDF_START.len() as u64)
        .read_to_end(&mut start)
        .map_err(cannot_read)?;
    if start != PDF_START {
        let detail = format!("{} does not start with %PDF-", pdf.display());
        return Err(refused("is not a PDF", detail));
    }
    Ok(())
}
