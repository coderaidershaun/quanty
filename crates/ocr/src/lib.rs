//! Turns the PDF of a book chapter, a paper or another document into saved pieces (headings,
//! text, formulas, figures, tables, code listings and footnotes) that later steps can read back
//! in reading order.

pub mod content;
pub mod convert;
pub mod reader;
#[cfg(feature = "testing")]
pub mod testing;

pub use content::{
    Catalogue, ChapterEntry, ChapterIndex, ChapterInsights, Cite, CiteKind, ContentError,
    DocumentName, FigureImage, ImageEntry, ImageIndex, ImageShows, Insight, MediaDocument, PageBox,
    PieceDetail, RelationshipKind, Symbol, gets_insights, saved_images,
};
pub use convert::{
    ChapterJob, ChapterLock, ConversionSummary, ConvertError, ConvertedImage, PageError,
    PageProgress, PopplerError, convert_chapter, convert_chapter_with_jev_key, convert_image,
    pdf_page_count, sha256_hex, try_lock_chapter,
};
pub use reader::{
    Chapter, ChapterPiece, FigurePicture, PieceId, PieceRelationship, ReadChapterError,
    SectionHeading, read_chapter,
};
