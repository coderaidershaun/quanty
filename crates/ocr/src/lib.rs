//! Turns a book chapter PDF into saved pieces (headings, text, formulas, figures, tables and
//! footnotes) that later steps can read back in reading order.
//!
//! [`convert_chapter`] converts the chapter a [`ChapterJob`] names and [`read_chapter`] reads it
//! back as a [`Chapter`]; both need only the crate root. `convert::services` and
//! `convert::reply` hold the paid calls, their errors and the reply types, for a caller that
//! swaps the paid calls for stand-ins.

pub mod content;
pub mod convert;
pub mod reader;

pub use content::{
    ChapterIndex, Cite, CiteKind, ContentError, FigureImage, ImageShows, PageBox, PieceDetail,
    RelationshipKind, Symbol,
};
pub use convert::{
    ChapterJob, ConversionSummary, ConvertError, PageError, PopplerError, convert_chapter,
};
pub use reader::{
    Chapter, ChapterPiece, FigurePicture, PieceId, PieceRelationship, ReadChapterError,
    SectionHeading, read_chapter,
};
