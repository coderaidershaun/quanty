//! Writes a document to one file that stands alone: a PDF, a Markdown file or an HTML file,
//! with its pictures inside it and its formulas typeset. The document is a title, a line that
//! says where it comes from, blocks of Markdown, formulas, figures, tables and code, and the
//! sources it cites.

mod commonmark;
mod document;
pub mod fonts;
mod html;
mod insights;
mod markdown;
mod math;
mod pdf;
mod pictures;

use std::path::{Path, PathBuf};

pub use document::{Block, Document, Format, Source};
pub use insights::{InsightsSheet, SheetInsight, insights_document};
pub use markdown::fence_for;

/// Why a document could not be written.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("could not write {}", path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not make the PDF: {reason}")]
    Pdf { reason: String },
}

/// Writes `document` as a file of `format` at `path`, and replaces the file when there is one.
///
/// # Errors
/// - [`ExportError::Pdf`] when the PDF cannot be made.
/// - [`ExportError::Write`] when the file cannot be written.
pub fn write(document: &Document, format: Format, path: &Path) -> Result<(), ExportError> {
    let bytes = match format {
        Format::Pdf => pdf::render(document)?,
        Format::Markdown => markdown::render(document).into_bytes(),
        Format::Html => html::render(document).into_bytes(),
    };
    std::fs::write(path, bytes).map_err(|source| ExportError::Write {
        path: path.to_owned(),
        source,
    })
}
