//! Turns the PDF of a book chapter, a paper or another document into saved pieces (headings,
//! text, formulas, figures, tables and footnotes) that later steps can read back in reading order.

pub mod content;
pub mod convert;
pub mod reader;
#[cfg(feature = "testing")]
pub mod testing;

pub use content::{
    Catalogue, ChapterEntry, ChapterIndex, Cite, CiteKind, ContentError, DocumentName, FigureImage,
    ImageIndex, ImageShows, MediaDocument, PageBox, PieceDetail, RelationshipKind, Symbol,
};
pub use convert::{
    ChapterJob, ConversionSummary, ConvertError, ConvertedImage, PageError, PageProgress,
    PopplerError, convert_chapter, convert_chapter_with_jev_key, convert_image, pdf_page_count,
};
pub use reader::{
    Chapter, ChapterPiece, FigurePicture, PieceId, PieceRelationship, ReadChapterError,
    SectionHeading, read_chapter,
};
