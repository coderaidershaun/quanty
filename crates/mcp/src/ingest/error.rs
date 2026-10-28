//! Everything that can stop an ingest: a PDF that is refused before anything is written, a job
//! that cannot start, and a job that fails while it runs.

use std::path::PathBuf;

use graph::GraphError;
use ocr::{ContentError, ConvertError};
use rag_core::{EmbedError, EmptyTag, StoreError, UnknownCategory};
use rag_ingestion::{NamePdfError, PdfError, RelabelError};
use reqwest::StatusCode;
use tokio::task::JoinError;

use crate::jobs::BackgroundJob;
use crate::jobs::{Busy, KEPT_JOBS, busy_from};

#[derive(thiserror::Error, Debug)]
pub(crate) enum PdfIngestError {
    #[error(
        "no PDF was given; give `path` (the absolute path of the PDF on the machine the server runs on), or `pdf_base64` with `file_name`, or `pdf_url` (an http or https address the server fetches the PDF from) with `file_name`"
    )]
    NoSource,

    #[error("{given} were given; give exactly one of them")]
    ManySources { given: &'static str },

    #[error(
        "`file_name` was given with `path`; `file_name` goes with `pdf_base64` or `pdf_url` only, because a file at a path has its own name"
    )]
    FileNameWithPath,

    #[error(
        "`{way}` was given without `file_name`; give the name to save it under, such as chapter-1-financial-contracts.pdf"
    )]
    MissingFileName { way: &'static str },

    #[error("`media` is blank; give the title of the media the PDF belongs to, such as a book")]
    BlankMedia,

    #[error("`category` is not valid")]
    Category(#[source] UnknownCategory),

    #[error(
        "a tag in `tags` or `document_tags` is blank; give each tag as a word, or leave the list out"
    )]
    Tag(#[source] EmptyTag),

    #[error("`path` must be an absolute path, and {} is not", path.display())]
    NotAbsolute { path: PathBuf },

    #[error(
        "could not read the PDF at {}; give the absolute path of a PDF file that is on the machine the server runs on",
        path.display()
    )]
    Unreadable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{} is not a file; give the absolute path of a PDF file", path.display())]
    NotAFile { path: PathBuf },

    #[error("the PDF is over the size limit of {}; send a smaller chapter", size_text(*limit))]
    TooBig { limit: u64 },

    #[error(
        "{} does not start with %PDF-, so it is not a PDF; give the path of a PDF file",
        path.display()
    )]
    NotAPdf { path: PathBuf },

    #[error(
        "the bytes of `pdf_base64` do not start with %PDF-, so they are not a PDF; send the bytes of a PDF file"
    )]
    UploadNotAPdf,

    #[error(
        "`pdf_url` {value:?} is not a URL ({reason}); give an address such as https://host/file.pdf"
    )]
    NotAUrl { value: String, reason: String },

    #[error(
        "`pdf_url` has the scheme {scheme:?}, and only http or https is fetched; give an address that starts with http:// or https://"
    )]
    PdfUrlScheme { scheme: String },

    #[error(
        "`pdf_url` points at {origin}, which is not an allowed origin: the server fetches only from its own machine (localhost, 127.0.0.1 or [::1]) and from the origins in QUANTY_FETCH_ORIGINS; send the PDF by `path` or `pdf_base64` instead, or ask whoever runs the server to add {origin} to QUANTY_FETCH_ORIGINS"
    )]
    PdfUrlOrigin { origin: String },

    #[error(
        "{url} answered {status}{}, and redirects are not followed; give the URL it points at",
        redirect_text(location.as_deref())
    )]
    PdfUrlRedirect {
        url: String,
        status: StatusCode,
        location: Option<String>,
    },

    #[error(
        "{url} answered {status}, not a PDF; check the address, or send the PDF by `path` or `pdf_base64`"
    )]
    PdfUrlStatus { url: String, status: StatusCode },

    #[error(
        "could not fetch the PDF from {url}; check that the address can be reached from the machine the server runs on"
    )]
    PdfUrlFetch {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error(
        "the answer of {url} does not start with %PDF-, so it is not a PDF; give the address of a PDF file"
    )]
    PdfUrlNotAPdf { url: String },

    #[error("could not set up the client that fetches `pdf_url`")]
    PdfUrlClient(#[source] reqwest::Error),

    #[error(
        "`file_name` must be a plain file name with no folder in it, and {name:?} is not; give a name such as chapter-1-financial-contracts.pdf"
    )]
    FileNameHoldsFolder { name: String },

    #[error(
        "`pdf_base64` is not standard base64 text; send the bytes of the PDF in the standard base64 alphabet, with `=` padding"
    )]
    Base64(#[source] base64::DecodeError),

    #[error(
        "`media` cannot name a folder; give the title of the media, with at least one letter or digit"
    )]
    Media(#[source] ContentError),

    #[error(
        "this media is a book, and the file name is not that of a book chapter; copy or rename the file to chapter-<number>-<name>.pdf, such as chapter-1-financial-contracts.pdf, or give such a `file_name` with `pdf_base64` or `pdf_url`, or give `document_title` when the PDF is a part of the book that is not a chapter, such as its index, or, when the library does not have this media yet, give a `category` of paper or other, whose PDF can have any name; a media the library has keeps its own category"
    )]
    ChapterFileName(#[source] ContentError),

    #[error(
        "`document_title` cannot name a folder; give a title with at least one letter or digit"
    )]
    DocumentTitle(#[source] ConvertError),

    #[error(
        "an ingest is running (job {}); ask `ingest_status` with that job_id, and send this PDF when that job has ended",
        .0.running_job
    )]
    Busy(Busy),

    #[error("background work is running: {0}; wait for it to end before sending this PDF")]
    SharedBusy(BackgroundJob),

    #[error(
        "no ingest job has the id {job_id:?}; job ids are forgotten when the server restarts, and the server keeps only its last {KEPT_JOBS} jobs, so send the PDF again: a PDF that is ingested is not ingested twice"
    )]
    UnknownJob { job_id: String },

    #[error(
        "the ingest job stopped before it reported; send the PDF again: converted pages and kept answers are not paid for twice"
    )]
    Lost,

    /// The finished text of a job that failed, which already says what to do.
    #[error("{0}")]
    Failed(String),

    #[error(
        "could not save the uploaded PDF at {}; check that the server can write to its content folder",
        path.display()
    )]
    Upload {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not set up the embedder")]
    Embedder(#[from] EmbedError),

    #[error("could not set up the item store")]
    ItemStore(#[source] StoreError),

    #[error("could not set up the concept store")]
    ConceptStore(#[source] StoreError),

    #[error("could not connect to the graph")]
    Graph(#[from] GraphError),

    #[error("could not read from the graph which media the library has")]
    ReadMedia(#[source] GraphError),

    #[error("could not ingest the PDF")]
    Pdf(#[source] Box<PdfError>),

    #[error("could not write the tags of the document")]
    Labels(#[from] RelabelError),

    #[error("the ingest stopped without an answer")]
    Stopped(#[from] JoinError),
}

busy_from!(PdfIngestError);

/// Each refusal of a name is worded for an agent, which sends arguments and not flags.
impl From<NamePdfError> for PdfIngestError {
    fn from(error: NamePdfError) -> PdfIngestError {
        match error {
            NamePdfError::ReadMedia(source) => PdfIngestError::ReadMedia(source),
            NamePdfError::ChapterFileName(source) => PdfIngestError::ChapterFileName(source),
        }
    }
}

fn redirect_text(location: Option<&str>) -> String {
    location.map_or_else(String::new, |location| {
        format!(" with the location {location}")
    })
}

fn size_text(bytes: u64) -> String {
    const MIB: u64 = 1024 * 1024;
    if bytes >= MIB && bytes.is_multiple_of(MIB) {
        format!("{} MiB", bytes / MIB)
    } else {
        format!("{bytes} bytes")
    }
}
