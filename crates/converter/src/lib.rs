//! Turns a book chapter PDF into saved pieces (headings, text, formulas, figures, tables and
//! footnotes) that later steps can read back in reading order.
//!
//! There are two ways in. [`convert_chapter`] converts the chapter a [`ChapterJob`] names.
//! [`read_chapter`] reads a converted chapter back as a [`Chapter`]. The errors of both, and
//! every type a [`ChapterPiece`] is made of, are exported here, so another crate imports from
//! the crate root alone.

pub mod categorise;
pub mod checks;
pub mod claude;
pub mod content;
pub mod convert;
pub mod jev;
mod page;
mod poppler;
pub mod reader;
mod save;
mod schema;
pub mod services;
pub mod summary;
pub mod transcribe;

pub use content::{
    ChapterIndex, Cite, CiteKind, ContentError, PieceDetail, RelationshipKind, Symbol,
};
pub use convert::{ChapterJob, ConvertError, convert_chapter};
pub use reader::{
    Chapter, ChapterPiece, PieceId, PieceRelationship, ReadChapterError, SectionHeading,
    read_chapter,
};
pub use summary::ConversionSummary;

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
