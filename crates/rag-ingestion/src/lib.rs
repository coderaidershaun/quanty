//! Turns a chapter PDF, a converted chapter, or a picture that stands alone, into stored,
//! searchable items and a graph of their concepts, and can relabel a stored media or document,
//! or delete a stored document or a whole media.

mod delete;
pub mod health;
mod ingest;
mod labels;
mod media;
mod pdf;
mod stores;
#[cfg(feature = "testing")]
pub mod testing;

pub use delete::{
    DeleteError, DocumentDeleteSummary, MediaDeleteSummary, delete_document, delete_media,
};
pub use ingest::{
    ASK_SCORE, ChapterFolder, ConceptError, ConceptExtractor, ConceptSummary, EXTRACTION_MODEL,
    IngestError, IngestStep, IngestSummary, Item, ItemCounts, LINK_SCORE, LoneImage, Models,
    SkippedItem, chapter_items, image_items, ingest_chapter, ingest_image, usage_of,
};
pub use labels::{RelabelError, Relabelled, TagChange, relabel_document_tags};
pub use media::{MediaChange, MediaRelabelled, relabel_media};
pub use pdf::{
    ChapterPdf, NamePdfError, NamedPdf, PdfError, PdfOutcome, PdfSummary, UnnamedPdf, ingest_pdf,
    items_of_ingested_document,
};
pub use stores::Stores;
