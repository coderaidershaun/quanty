//! Checks the one PDF that an agent sent, before anything is written, stored or paid for: where it
//! comes from, how big it is, that it is a PDF, where an uploaded or fetched one is saved, and how
//! it is named once the category of its media is known.

use std::collections::BTreeSet;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use graph::FalkorGraph;
use ocr::ChapterJob;
use ocr::content::media_folder_name;
use rag_core::{Category, Config, MediaLabels, Tag, author_list};
use rag_ingestion::{TagChange, UnnamedPdf, uploaded_pdf_path};
use tempfile::TempDir;

use super::fetch::fetch_pdf;
use super::{IngestPdfArgs, PDF_START, PdfIngestError};
use crate::args::{file_name_of, non_blank, parse_optional, parse_tags};

/// A PDF whose arguments and file passed every check. It is not named yet: that needs the
/// category of its media, which the library may already have.
pub(super) struct CheckedPdf {
    pdf_path: PathBuf,
    media: String,
    /// The category the agent gave. It makes a new media only.
    category: Option<Category>,
    document_title: Option<String>,
    file_name: String,
    upload: Option<Upload>,
    /// The authors and the tags that the media is made with when the library does not have it yet.
    authors: Vec<String>,
    media_tags: BTreeSet<Tag>,
    document_tags: TagChange,
}

/// A checked PDF with its name, ready for a job.
pub(super) struct ReadyPdf {
    pub(super) chapter: ChapterJob,
    pub(super) media: String,
    pub(super) file_name: String,
    /// The bytes to save first, when the PDF was sent as base64 or fetched from a URL.
    pub(super) upload: Option<Upload>,
    /// The labels the media is made with when the library does not have it yet.
    pub(super) new_media: MediaLabels,
    pub(super) document_tags: TagChange,
}

pub(super) struct Upload {
    pub(super) bytes: Vec<u8>,
    pub(super) save_to: PathBuf,
}

/// The PDF of the one way it was sent.
struct Source {
    pdf_path: PathBuf,
    file_name: String,
    upload: Option<Upload>,
}

pub(crate) fn base64_len(bytes: u64) -> u64 {
    bytes.div_ceil(3) * 4
}

/// The ways an agent can send the PDF, of which exactly one must be given, and the name to save
/// it under.
struct Sent {
    path: Option<String>,
    pdf_base64: Option<String>,
    pdf_url: Option<String>,
    file_name: Option<String>,
}

/// Checks the arguments, and reads the PDF when it was sent as base64 or by URL. It writes nothing.
///
/// # Errors
/// The first check that fails, with what was given and what is allowed.
pub(super) async fn check(
    args: IngestPdfArgs,
    config: &Config,
    max_pdf_bytes: u64,
) -> Result<CheckedPdf, PdfIngestError> {
    let media = non_blank(Some(args.media)).ok_or(PdfIngestError::BlankMedia)?;
    let category = parse_optional::<Category>(args.category).map_err(PdfIngestError::Category)?;
    // The media is checked here, so that the checks below can fail for the document only.
    let media_folder = media_folder_name(&media).map_err(PdfIngestError::Media)?;
    let media_tags = parse_tags(args.tags).map_err(PdfIngestError::Tag)?;
    let document_tags = TagChange {
        add: parse_tags(args.document_tags).map_err(PdfIngestError::Tag)?,
        remove: Vec::new(),
    };
    let sent = Sent {
        path: non_blank(args.path),
        // The base64 text can be megabytes long, so it is not trimmed or copied here.
        pdf_base64: args.pdf_base64.filter(|text| !text.trim().is_empty()),
        pdf_url: non_blank(args.pdf_url),
        file_name: non_blank(args.file_name),
    };
    let source = checked_source(sent, config, &media_folder, max_pdf_bytes).await?;
    Ok(CheckedPdf {
        pdf_path: source.pdf_path,
        media,
        category,
        document_title: non_blank(args.document_title),
        file_name: source.file_name,
        upload: source.upload,
        authors: author_list(args.authors.unwrap_or_default()),
        media_tags,
        document_tags,
    })
}

/// Exactly one way must be given. A PDF sent as base64 is read here, and one named by URL is
/// fetched here; both are saved later.
async fn checked_source(
    sent: Sent,
    config: &Config,
    media_folder: &str,
    max_pdf_bytes: u64,
) -> Result<Source, PdfIngestError> {
    let Sent {
        path,
        pdf_base64,
        pdf_url,
        file_name,
    } = sent;
    match (path, pdf_base64, pdf_url) {
        (None, None, None) => Err(PdfIngestError::NoSource),
        (Some(path), None, None) => {
            if file_name.is_some() {
                return Err(PdfIngestError::FileNameWithPath);
            }
            // Reading the disk can block, so it runs on a thread that is allowed to block.
            let pdf_path = blocking(move || path_of_a_pdf(&path, max_pdf_bytes)).await?;
            let file_name = file_name_of(&pdf_path).unwrap_or_default();
            Ok(Source {
                pdf_path,
                file_name,
                upload: None,
            })
        }
        (None, Some(text), None) => {
            let (file_name, save_to) =
                upload_target("pdf_base64", file_name, config, media_folder)?;
            // The text can be megabytes long, so it is decoded on a thread that may block and the
            // other calls are not held up.
            let bytes = blocking(move || decoded_pdf(&text, max_pdf_bytes)).await?;
            Ok(uploaded(file_name, save_to, bytes))
        }
        (None, None, Some(url)) => {
            let (file_name, save_to) = upload_target("pdf_url", file_name, config, media_folder)?;
            let bytes = fetch_pdf(&url, &config.fetch_origins, max_pdf_bytes).await?;
            Ok(uploaded(file_name, save_to, bytes))
        }
        (Some(_), Some(_), None) => Err(PdfIngestError::ManySources {
            given: "both `path` and `pdf_base64`",
        }),
        (Some(_), None, Some(_)) => Err(PdfIngestError::ManySources {
            given: "both `path` and `pdf_url`",
        }),
        (None, Some(_), Some(_)) => Err(PdfIngestError::ManySources {
            given: "both `pdf_base64` and `pdf_url`",
        }),
        (Some(_), Some(_), Some(_)) => Err(PdfIngestError::ManySources {
            given: "all of `path`, `pdf_base64` and `pdf_url`",
        }),
    }
}

/// The file name and the place to save a PDF that is not on the disk yet. A bad name is refused
/// here, before the PDF is read or fetched.
fn upload_target(
    way: &'static str,
    file_name: Option<String>,
    config: &Config,
    media_folder: &str,
) -> Result<(String, PathBuf), PdfIngestError> {
    let file_name = file_name.ok_or(PdfIngestError::MissingFileName { way })?;
    let save_to = uploaded_pdf_path(&config.content_folder, media_folder, &file_name)
        .map_err(|error| PdfIngestError::FileNameHoldsFolder { name: error.name })?;
    Ok((file_name, save_to))
}

fn uploaded(file_name: String, save_to: PathBuf, bytes: Vec<u8>) -> Source {
    Source {
        pdf_path: save_to.clone(),
        file_name,
        upload: Some(Upload { bytes, save_to }),
    }
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, PdfIngestError> + Send + 'static,
) -> Result<T, PdfIngestError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(PdfIngestError::Stopped)?
}

impl CheckedPdf {
    /// Writes the bytes of an uploaded or fetched PDF to a temporary folder, under their own file
    /// name, and points the PDF there instead of at the place a job saves it. The folder goes away
    /// when the returned value is dropped. A PDF that is on the disk already is left alone and
    /// gives `None`.
    ///
    /// # Errors
    /// [`PdfIngestError::Temporary`] when the folder or the file cannot be written.
    pub(super) async fn keep_upload_in_a_temporary_folder(
        &mut self,
    ) -> Result<Option<TempDir>, PdfIngestError> {
        let Some(upload) = self.upload.take() else {
            return Ok(None);
        };
        let folder = tempfile::tempdir().map_err(|source| PdfIngestError::Temporary {
            path: std::env::temp_dir(),
            source,
        })?;
        let kept = folder.path().join(&self.file_name);
        let written = kept.clone();
        // Writing megabytes to the disk can take a while, so it runs on a thread that may block.
        blocking(move || {
            std::fs::write(&written, &upload.bytes).map_err(|source| PdfIngestError::Temporary {
                path: written,
                source,
            })
        })
        .await?;
        self.pdf_path = kept;
        Ok(Some(folder))
    }

    /// Names the PDF by the category that the graph has for its media, or else by the one the
    /// agent gave. It writes nothing.
    ///
    /// # Errors
    /// - [`PdfIngestError::ReadMedia`] when the graph cannot list its media
    /// - [`PdfIngestError::ChapterFileName`] when a book's PDF has no document title and is not
    ///   named as a chapter
    /// - [`PdfIngestError::DocumentTitle`] when the document title cannot name a folder
    pub(super) async fn named(
        self,
        graph: &FalkorGraph,
        config: &Config,
    ) -> Result<ReadyPdf, PdfIngestError> {
        let unnamed = UnnamedPdf {
            media_title: &self.media,
            category: self.category,
            document_title: self.document_title.as_deref(),
            pdf: &self.pdf_path,
        };
        let named = unnamed.named(graph).await?;
        let chapter = ChapterJob::new(named.document, &self.pdf_path, &config.content_folder)
            .map_err(PdfIngestError::DocumentTitle)?;
        Ok(ReadyPdf {
            chapter,
            media: self.media,
            file_name: self.file_name,
            upload: self.upload,
            new_media: MediaLabels {
                category: named.category,
                authors: self.authors,
                tags: self.media_tags,
            },
            document_tags: self.document_tags,
        })
    }
}

fn path_of_a_pdf(path: &str, max_pdf_bytes: u64) -> Result<PathBuf, PdfIngestError> {
    let path = PathBuf::from(path);
    if !path.is_absolute() {
        return Err(PdfIngestError::NotAbsolute { path });
    }
    let unreadable = |source| PdfIngestError::Unreadable {
        path: path.clone(),
        source,
    };
    let metadata = std::fs::metadata(&path).map_err(unreadable)?;
    if !metadata.is_file() {
        return Err(PdfIngestError::NotAFile { path });
    }
    if metadata.len() > max_pdf_bytes {
        return Err(PdfIngestError::TooBig {
            limit: max_pdf_bytes,
        });
    }
    let mut start = Vec::new();
    File::open(&path)
        .and_then(|file| file.take(PDF_START.len() as u64).read_to_end(&mut start))
        .map_err(unreadable)?;
    if start != PDF_START {
        return Err(PdfIngestError::NotAPdf { path });
    }
    Ok(path)
}

fn decoded_pdf(text: &str, max_pdf_bytes: u64) -> Result<Vec<u8>, PdfIngestError> {
    // The length is checked before the text is decoded, so a huge text costs no memory.
    if text.len() as u64 > base64_len(max_pdf_bytes) {
        return Err(PdfIngestError::TooBig {
            limit: max_pdf_bytes,
        });
    }
    let bytes = STANDARD
        .decode(text.trim())
        .map_err(PdfIngestError::Base64)?;
    if bytes.len() as u64 > max_pdf_bytes {
        return Err(PdfIngestError::TooBig {
            limit: max_pdf_bytes,
        });
    }
    if !bytes.starts_with(PDF_START) {
        return Err(PdfIngestError::UploadNotAPdf);
    }
    Ok(bytes)
}
