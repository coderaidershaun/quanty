//! Maps converted chapters to items and checks what each item stores and what it gives the
//! embedder.

mod chapters;
mod chunks;

use ocr::{Chapter, PieceId};
use rag_core::ItemKind;
use rag_ingestion::Item;

fn of_kind(items: &[Item], kind: ItemKind) -> Vec<&Item> {
    items
        .iter()
        .filter(|item| item.payload.kind == kind)
        .collect()
}

fn content_of(chapter: &Chapter, page: u32, number: u32) -> &str {
    &chapter.piece(PieceId { page, number }).unwrap().content
}
