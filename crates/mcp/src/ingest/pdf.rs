//! Checks the one PDF that an agent sent, before anything is written, stored or paid for: where it
//! comes from, how big it is, that it is a PDF, and where an uploaded one is saved.

use std::ffi::OsStr;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use ocr::content::media_folder_name;
use ocr::{ChapterJob, MediaDocument};
use rag_core::{Category, Config, MediaLabels, Tag, author_list};
use rag_ingestion::{TagChange, document_name};

use super::{IngestPdfArgs, PdfIngestError};

const PDF_START: &[u8] = b"%PDF-";

/// The folder under the content folder that uploads are saved in. A media folder name never holds
/// an underscore, so no upload can land in the folder of a media.
const UPLOADS_FOLDER: &str = "_uploads";

pub(super) struct CheckedPdf {
    pub(super) chapter: ChapterJob,
    pub(super) media: String,
    pub(super) file_name: String,
    /// The bytes to save first, when the PDF was sent as base64.
    pub(super) upload: Option<Upload>,
    /// The labels the media is made with when the library does not have it yet.
    pub(super) new_media: MediaLabels,
    pub(super) document_tags: TagChange,
}

pub(super) struct Upload {
    pub(super) bytes: Vec<u8>,
    pub(super) save_to: PathBuf,
}

pub(crate) fn base64_len(bytes: u64) -> u64 {
    bytes.div_ceil(3) * 4
}

/// Checks the arguments, and reads the PDF when it was sent as base64. It writes nothing.
///
/// # Errors
/// The first check that fails, with what was given and what is allowed.
pub(super) fn check(
    args: IngestPdfArgs,
    config: &Config,
    max_pdf_bytes: u64,
) -> Result<CheckedPdf, PdfIngestError> {
    let media = non_blank(Some(args.media)).ok_or(PdfIngestError::BlankMedia)?;
    let category = non_blank(args.category)
        .map(|category| category.parse::<Category>())
        .transpose()
        .map_err(PdfIngestError::Category)?
        .unwrap_or_default();
    // The media is checked here, so that the checks below can fail for the document only.
    let media_folder = media_folder_name(&media).map_err(PdfIngestError::Media)?;
    let document_title = non_blank(args.document_title);
    if category == Category::Book && document_title.is_some() {
        return Err(PdfIngestError::TitleForABook);
    }
    let path = non_blank(args.path);
    // The base64 text can be megabytes long, so it is not trimmed or copied here.
    let pdf_base64 = args.pdf_base64.filter(|text| !text.trim().is_empty());
    let file_name = non_blank(args.file_name);
    let new_media = MediaLabels {
        category,
        authors: author_list(args.authors.unwrap_or_default()),
        tags: tags_of(args.tags)?.into_iter().collect(),
    };
    let document_tags = TagChange {
        add: tags_of(args.document_tags)?,
        remove: Vec::new(),
    };

    let (pdf_path, file_name, upload) = match (path, pdf_base64) {
        (None, None) => return Err(PdfIngestError::NoSource),
        (Some(_), Some(_)) => return Err(PdfIngestError::TwoSources),
        (Some(path), None) => {
            if file_name.is_some() {
                return Err(PdfIngestError::FileNameWithPath);
            }
            let path = path_of_a_pdf(&path, max_pdf_bytes)?;
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            (path, name, None)
        }
        (None, Some(text)) => {
            let name = file_name.ok_or(PdfIngestError::MissingFileName)?;
            let save_to = upload_path(config, &media_folder, &name)?;
            let bytes = decoded_pdf(&text, max_pdf_bytes)?;
            let upload = Upload {
                bytes,
                save_to: save_to.clone(),
            };
            (save_to, name, Some(upload))
        }
    };
    let title = document_title.as_deref().unwrap_or(&media);
    let name =
        document_name(category, title, &pdf_path).map_err(PdfIngestError::ChapterFileName)?;
    let document = MediaDocument {
        media_title: media.clone(),
        name,
    };
    let chapter = ChapterJob::new(document, &pdf_path, &config.content_folder)
        .map_err(PdfIngestError::DocumentTitle)?;
    Ok(CheckedPdf {
        chapter,
        media,
        file_name,
        upload,
        new_media,
        document_tags,
    })
}

fn tags_of(texts: Option<Vec<String>>) -> Result<Vec<Tag>, PdfIngestError> {
    texts
        .unwrap_or_default()
        .iter()
        .map(|tag| tag.parse::<Tag>())
        .collect::<Result<_, _>>()
        .map_err(PdfIngestError::Tag)
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

/// Where an uploaded PDF is saved: never a path that the caller chose, only a file name that
/// has no folder in it, under the folder of the media under the uploads folder.
fn upload_path(
    config: &Config,
    media_folder: &str,
    file_name: &str,
) -> Result<PathBuf, PdfIngestError> {
    let is_plain = Path::new(file_name)
        .file_name()
        .is_some_and(|plain| plain == OsStr::new(file_name));
    if !is_plain {
        return Err(PdfIngestError::FileNameHoldsFolder {
            name: file_name.to_owned(),
        });
    }
    Ok(config
        .content_folder
        .join(UPLOADS_FOLDER)
        .join(media_folder)
        .join(file_name))
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

fn non_blank(text: Option<String>) -> Option<String> {
    text.map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}
