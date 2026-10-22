//! Every formula, figure, table and code listing of one document, read from its chapter folder on
//! disk, so the Library shows them while the stores are down.

use crate::contract::ids::DocId;
use crate::contract::source::{PagePiece, PieceKind};

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
    /// Accepted source-valid display metadata, independent of the printed piece.
    pub metadata: Option<ArtifactMetadata>,
}

/// Common Library title and purpose above the original content.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ArtifactMetadata {
    pub title: String,
    pub description: String,
}

impl ChapterItem {
    /// The page it is printed on, as a result row names it: "p. 7", "pp. 7–8" for a listing that
    /// page breaks cut, and "page 3" or "pages 3–4" by position when the page shows no number.
    pub fn page_label(&self) -> String {
        // SMELL: the result rows of an answer write their pages with their own copy of these
        // words, so a change here must be made there too.
        match (&self.printed_page, self.last_page, &self.last_printed_page) {
            (Some(first), Some(_), Some(last)) => format!("pp. {first}\u{2013}{last}"),
            (_, Some(last_page), _) => format!("pages {}\u{2013}{last_page}", self.page),
            (Some(printed), None, _) => format!("p. {printed}"),
            (None, None, _) => format!("page {}", self.page),
        }
    }
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
