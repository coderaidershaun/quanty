//! Everything that can stop an ingest: a PDF that is refused before anything is written, a job
//! that cannot start, and a job that fails while it runs.

use std::path::PathBuf;

use graph::GraphError;
use ocr::{ContentError, ConvertError};
use rag_core::{EmbedError, EmptyTag, StoreError};
use rag_ingestion::{PdfError, RelabelError};
use tokio::task::JoinError;

use super::jobs::KEPT_JOBS;

#[derive(thiserror::Error, Debug)]
pub(crate) enum PdfIngestError {
    #[error(
        "no PDF was given; give `path` (the absolute path of the PDF on the machine the server runs on) or `pdf_base64` with `file_name`"
    )]
    NoSource,

    #[error("both `path` and `pdf_base64` were given; give one of them")]
    TwoSources,

    #[error(
        "`file_name` was given with `path`; `file_name` goes with `pdf_base64` only, because a file at a path has its own name"
    )]
    FileNameWithPath,

    #[error(
        "`pdf_base64` was given without `file_name`; give the name to save it under, such as chapter-1-financial-contracts.pdf"
    )]
    MissingFileName,

    #[error("`book` is blank; give the title of the book the chapter is from")]
    BlankBook,

    #[error("a tag in `tags` is blank; give each tag as a word, or leave `tags` out")]
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
        "`file_name` must be a plain file name with no folder in it, and {name:?} is not; give a name such as chapter-1-financial-contracts.pdf"
    )]
    FileNameHoldsFolder { name: String },

    #[error(
        "`pdf_base64` is not standard base64 text; send the bytes of the PDF in the standard base64 alphabet, with `=` padding"
    )]
    Base64(#[source] base64::DecodeError),

    #[error(
        "`book` cannot name a folder; give the title of the book, with at least one letter or digit"
    )]
    Book(#[source] ContentError),

    #[error(
        "the file name is not that of a chapter; copy or rename the file to chapter-<number>-<name>.pdf, such as chapter-1-financial-contracts.pdf, or give such a `file_name` with `pdf_base64`"
    )]
    Chapter(#[source] ConvertError),

    #[error(
        "an ingest is running (job {job_id}); ask `ingest_status` with that job_id, and send this PDF when that job has ended"
    )]
    Busy { job_id: String },

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

    #[error("could not ingest the PDF")]
    Pdf(#[source] Box<PdfError>),

    #[error("could not write the author and the tags")]
    Labels(#[from] RelabelError),

    #[error("the ingest stopped without an answer")]
    Stopped(#[source] JoinError),
}

/// A limit that is a whole number of MiB reads as that number, any other as bytes.
fn size_text(bytes: u64) -> String {
    const MIB: u64 = 1024 * 1024;
    if bytes >= MIB && bytes.is_multiple_of(MIB) {
        format!("{} MiB", bytes / MIB)
    } else {
        format!("{bytes} bytes")
    }
}
