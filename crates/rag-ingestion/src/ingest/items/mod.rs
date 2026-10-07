//! Decides what a chapter, or a picture that stands alone, is stored as: which pieces become
//! items, what each item keeps and what each one gives the embedder. It reads no file and calls
//! nothing outside.

mod chunks;
mod image;
mod pieces;

use ocr::{Chapter, ChapterIndex, PieceId, SectionHeading};
use rag_core::{DocId, DocumentInput, ItemId, ItemKind, ItemPayload};

pub use image::{LoneImage, image_items};

/// Blocks of one item, such as a lead-in and a formula, are told apart by a blank line.
const BLOCK_SEPARATOR: &str = "\n\n";

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub id: ItemId,
    /// What is stored.
    pub payload: ItemPayload,
    /// What is embedded.
    pub input: DocumentInput,
}

/// An item that has no place in the chapter's list yet, so no identifier either.
struct Draft {
    /// Where the item starts. Items are put in reading order by it.
    first_piece: PieceId,
    printed_page: Option<String>,
    kind: ItemKind,
    text: String,
    label: Option<String>,
    cites: Vec<String>,
    input: DocumentInput,
}

impl Draft {
    fn into_item(self, document: DocId, doc_title: &str, position: u32) -> Item {
        Item {
            id: ItemId::new(document, self.kind, position),
            payload: ItemPayload {
                doc_id: document,
                doc_title: doc_title.to_owned(),
                page: self.first_piece.page,
                printed_page: self.printed_page,
                kind: self.kind,
                text: self.text,
                image_path: self.input.image.clone(),
                label: self.label,
                cites: self.cites,
            },
            input: self.input,
        }
    }
}

/// Every item of the chapter in reading order, each placed by the piece it starts at. A chunk
/// made of text pieces 1, 3 and 5 therefore comes before the formulas at 2 and 4.
pub fn chapter_items(chapter: &Chapter) -> Vec<Item> {
    let document = document_id(&chapter.index);
    let doc_title = document_title(&chapter.index);

    let mut drafts = chunks::chunk_drafts(chapter, &doc_title);
    drafts.extend(
        chapter
            .pieces
            .iter()
            .filter_map(|piece| pieces::piece_draft(chapter, piece, &doc_title)),
    );
    drafts.sort_by_key(|draft| draft.first_piece);

    (0u32..)
        .zip(drafts)
        .map(|(position, draft)| draft.into_item(document, &doc_title, position))
        .collect()
}

pub(super) fn document_id(index: &ChapterIndex) -> DocId {
    DocId::from_source_sha256(&index.source_sha256)
}

pub(super) fn document_title(index: &ChapterIndex) -> String {
    format!(
        "{}, chapter {}: {}",
        index.book_title, index.chapter_number, index.chapter_name
    )
}

/// The document title followed by the headings the piece sits under, outermost first.
fn context_line(doc_title: &str, section: &[SectionHeading]) -> String {
    let headings = section.iter().map(heading_text);
    std::iter::once(doc_title.to_owned())
        .chain(headings)
        .collect::<Vec<_>>()
        .join(" > ")
}

fn heading_text(heading: &SectionHeading) -> String {
    match &heading.printed_number {
        Some(number) => format!("{number} {}", heading.text),
        None => heading.text.clone(),
    }
}
