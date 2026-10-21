//! A document as a PDF made in Rust: pages of A4 with the text set in the app's own fonts and
//! the formulas as pictures. Every block is laid out into chunks first, the chunks are stacked
//! down the pages, and only then is each page drawn, so that a page can say how many there are.

mod blocks;
mod breaker;
mod faces;
mod flow;
mod pages;
mod pieces;
mod read;
mod table;

use krilla::Document as Pdf;
use krilla::metadata::Metadata;

use self::faces::Faces;
use self::flow::Layout;
use crate::{Document, ExportError};

const PAGE_WIDTH: f32 = 595.28;
const PAGE_HEIGHT: f32 = 841.89;
const MARGIN: f32 = 72.0;
const TEXT_WIDTH: f32 = PAGE_WIDTH - 2.0 * MARGIN;
const CONTENT_HEIGHT: f32 = PAGE_HEIGHT - 2.0 * MARGIN;

pub(crate) fn render(document: &Document) -> Result<Vec<u8>, ExportError> {
    let faces = Faces::new()?;
    let chunks = Layout::new(&faces).document(document);
    let mut pdf = Pdf::new();
    pdf.set_metadata(Metadata::new().title(document.title.clone()));
    pages::draw(&mut pdf, &faces, pages::paginate(chunks));
    pdf.finish().map_err(|error| ExportError::Pdf {
        reason: error.to_string(),
    })
}
