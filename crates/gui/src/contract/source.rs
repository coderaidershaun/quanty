//! A page of a chapter as it was saved on disk: its pictures and its pieces in reading order,
//! and the concepts that appear on it.

use super::ids::{ConceptId, DocId};
use super::library::ChapterLabel;

/// An absolute path to a PNG or JPEG. Also the key of the picture cache.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ImageRef {
    pub path: std::path::PathBuf,
}

/// A rectangle on a page, in thousandths of its width and height.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct PageBox {
    pub left: u16,
    pub top: u16,
    pub right: u16,
    pub bottom: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PieceKind {
    Heading { rank: u8 },
    Text,
    Formula,
    Figure,
    Table,
    Footnote,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PagePiece {
    pub number: u32,
    pub kind: PieceKind,
    /// The printed label. For a heading its printed number, for a footnote its marker.
    pub label: Option<String>,
    pub name: Option<String>,
    pub caption: Option<String>,
    pub text: String,
    pub image: Option<ImageRef>,
    /// Where a figure was cut from the page. Only when the cut shows the figure.
    pub cut: Option<PageBox>,
}

/// Everything about one page. It is read from disk, so it works while the stores are down.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct PageView {
    pub doc: DocId,
    pub page: u32,
    pub book: Option<String>,
    pub chapter: Option<ChapterLabel>,
    pub page_count: u32,
    pub printed_page: Option<String>,
    /// `None`: the chapter has no page pictures, as a hand-written chapter has none.
    pub image: Option<ImageRef>,
    /// The neighbouring pages' pictures, so the source view can load them ahead of time.
    pub previous_image: Option<ImageRef>,
    pub next_image: Option<ImageRef>,
    pub pieces: Vec<PagePiece>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct PageConcept {
    pub id: ConceptId,
    pub name: String,
    pub definition: String,
}
