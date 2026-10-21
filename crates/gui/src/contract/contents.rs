//! Every formula, figure, table and code listing of one document, read from its chapter folder on
//! disk, so the Library shows them while the stores are down.

use super::ids::DocId;
use super::source::{PagePiece, PieceKind};

/// One item with the page it is on. A code listing that page breaks cut is one item: its
/// `piece.text` is the whole listing, and `last_page` is the page it ends on.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChapterItem {
    /// The position of the page the item starts on, from 1.
    pub page: u32,
    pub printed_page: Option<String>,
    /// The position of the page the item ends on, only when it is later than `page`.
    pub last_page: Option<u32>,
    pub last_printed_page: Option<String>,
    /// `piece.number` is its number on `page`, so `Intent::OpenSource { doc, page, piece:
    /// Some(number) }` opens it.
    pub piece: PagePiece,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ChapterContents {
    pub doc: DocId,
    pub page_count: u32,
    /// In reading order: by page, then by number on the page. Only formulas, figures, tables and
    /// code.
    pub items: Vec<ChapterItem>,
}

impl ChapterContents {
    pub fn of_kind(&self, kind: PieceKind) -> impl Iterator<Item = &ChapterItem> {
        self.items
            .iter()
            .filter(move |item| item.piece.kind == kind)
    }
}
