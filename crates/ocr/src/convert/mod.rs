//! Converts a whole chapter PDF into its folder of pages: cut the pages out, convert them a few
//! at a time, and write `chapter.json` last. A finished chapter is never converted again.

mod chapter;
mod checks;
mod figure;
mod image;
mod page;
mod poppler;
pub mod reply;
mod route;
mod save;
pub mod services;
mod summary;
mod usable_box;

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use tokio::process::Command;

use crate::content::{
    ChapterIndex, ContentError, DocumentName, FORMAT_VERSION, MediaDocument, PageIndex,
    media_folder_name, page_folder_name,
};

use services::{ClaudeError, LiveServices, PageServices, ServiceError};

pub use chapter::{ChapterLock, try_lock_chapter};
pub use checks::{PieceRef, ReplyFault};
pub use image::{ConvertedImage, convert_image, convert_image_with};
pub use page::PageError;
pub use poppler::PopplerError;
pub use summary::{
    CallTally, ConversionSummary, ModelCalls, OutOfSequence, PageProgress, PageToCheck,
    PieceCounts, RouteCounts,
};

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

    #[error(
        "another run is converting {} at this moment; wait for it to finish, then run again",
        folder.display()
    )]
    ChapterBusy { folder: PathBuf },

    #[error(transparent)]
    Poppler(#[from] PopplerError),

    #[error("the outside services could not be started")]
    Services(#[from] ServiceError),

    #[error(
        "{} is not a picture that can be read: a picture must be a PNG or a JPEG, judging by its file name",
        path.display()
    )]
    NotAPicture { path: PathBuf },

    #[error("could not read the picture {}", path.display())]
    PictureUnreadable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("the model could not be asked to explain the picture {}", picture.display())]
    ImageCallFailed {
        picture: PathBuf,
        #[source]
        source: ServiceError,
    },

    #[error("the reply for the picture {} was rejected twice", picture.display())]
    ImageReplyRejected {
        picture: PathBuf,
        /// What was wrong with the second reply.
        #[source]
        fault: ReplyFault,
    },

    #[error("page {position} could not be converted (its working folder is {})", folder.display())]
    PageFailed {
        position: u32,
        folder: PathBuf,
        #[source]
        source: Box<PageError>,
    },
}

#[derive(Debug, Clone)]
pub struct ChapterJob {
    media_title: String,
    name: DocumentName,
    chapter_pdf: PathBuf,
    source_file_name: String,
    chapter_folder: PathBuf,
}

impl ChapterJob {
    /// Works out the document's folder under `output_root`. Opens no file, and the name of the
    /// document comes from `document`, never from the file's name.
    ///
    /// # Errors
    /// [`ConvertError::Content`] when the media title or the document title has no letters or
    /// digits.
    pub fn new(
        document: MediaDocument,
        chapter_pdf: &Path,
        output_root: &Path,
    ) -> Result<Self, ConvertError> {
        let source_file_name = chapter_pdf
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let chapter_folder = output_root
            .join(media_folder_name(&document.media_title)?)
            .join(document.name.folder_name()?);
        Ok(Self {
            media_title: document.media_title,
            name: document.name,
            chapter_pdf: chapter_pdf.to_path_buf(),
            source_file_name,
            chapter_folder,
        })
    }

    /// `<output root>/<media folder>/<document folder>`.
    pub fn chapter_folder(&self) -> PathBuf {
        self.chapter_folder.clone()
    }

    /// The SHA-256 of the chapter PDF as lower-case hex: the value that `chapter.json` keeps as
    /// `source-sha256`. It reads the whole file.
    ///
    /// # Errors
    /// [`ConvertError::SourceUnreadable`] when the PDF cannot be read.
    pub fn source_sha256(&self) -> Result<String, ConvertError> {
        let bytes =
            std::fs::read(&self.chapter_pdf).map_err(|source| ConvertError::SourceUnreadable {
                path: self.chapter_pdf.clone(),
                source,
            })?;
        Ok(sha256_hex(&bytes))
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
/// - [`ConvertError::ChapterBusy`] if another run is converting the same chapter
/// - [`ConvertError::PageFailed`] for the lowest page that failed; finished pages stay saved
pub async fn convert_chapter(job: &ChapterJob) -> Result<ConversionSummary, ConvertError> {
    let jev_api_key = std::env::var(services::JEV_API_KEY_VARIABLE).ok();
    convert_chapter_with_jev_key(job, jev_api_key.as_deref(), |_| {}).await
}

/// Like [`convert_chapter`], with the Jev key given by the caller and not read from the
/// environment, for a program that keeps its settings out of the process environment. `None` is
/// an error only when a page is left to convert.
///
/// `on_progress` hears the page count first, then each page as it is cut out, then each page as
/// it ends. A finished chapter hears nothing.
///
/// # Errors
/// The same as [`convert_chapter`]. A key that is `None` gives [`ConvertError::Services`].
pub async fn convert_chapter_with_jev_key(
    job: &ChapterJob,
    jev_api_key: Option<&str>,
    on_progress: impl FnMut(PageProgress),
) -> Result<ConversionSummary, ConvertError> {
    if services::api_key_is_set() {
        return Err(ConvertError::ApiKeySet);
    }
    match prepare(job)? {
        Prepared::Finished(summary) => Ok(summary),
        Prepared::ToDo { source_sha256 } => {
            let services = LiveServices::with_jev_key(jev_api_key).await?;
            chapter::run(job, &source_sha256, &services, on_progress).await
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
    convert_chapter_with_progress(job, services, |_| {}).await
}

/// Like [`convert_chapter_with`], and `on_progress` hears the page count first, then each page
/// as it is cut out, then each page as it ends. A finished chapter hears nothing.
///
/// # Errors
/// The same as [`convert_chapter_with`].
pub async fn convert_chapter_with_progress<S: PageServices>(
    job: &ChapterJob,
    services: &S,
    on_progress: impl FnMut(PageProgress),
) -> Result<ConversionSummary, ConvertError> {
    match prepare(job)? {
        Prepared::Finished(summary) => Ok(summary),
        Prepared::ToDo { source_sha256 } => {
            chapter::run(job, &source_sha256, services, on_progress).await
        }
    }
}

/// The number of pages of a PDF, read with `pdfinfo`. It costs nothing and writes nothing.
///
/// # Errors
/// [`PopplerError`] when `pdfinfo` cannot run or reads no page count from the file.
pub async fn pdf_page_count(pdf: &Path) -> Result<u32, PopplerError> {
    poppler::page_count(pdf).await
}

/// The SHA-256 of `bytes` as lower-case hex: the text that `chapter.json` and `image.json` keep
/// as `source-sha256`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Reads the PDF and the chapter folder and decides what is left to do. Writes nothing.
fn prepare(job: &ChapterJob) -> Result<Prepared, ConvertError> {
    let source_sha256 = job.source_sha256()?;
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

/// Start every child process of a conversion here: one started any other way inherits the Jev
/// API key, which only the Jev client needs.
fn child_process(program: &str) -> Command {
    let mut command = Command::new(program);
    command.env_remove(services::JEV_API_KEY_VARIABLE);
    command
}

fn write_error(path: &Path) -> impl FnOnce(std::io::Error) -> ContentError + '_ {
    |source| ContentError::Write {
        path: path.to_path_buf(),
        source,
    }
}
