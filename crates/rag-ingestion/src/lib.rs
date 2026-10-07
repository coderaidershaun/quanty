//! Turns a converted chapter, or a picture that stands alone, into stored, searchable items and a
//! graph of its document, items and the concepts they discuss, removes a document from the
//! stores, and checks that the services this needs are ready.

mod delete;
pub mod health;
mod ingest;
mod stores;

pub use delete::{DeleteError, DeleteSummary, delete_document};
pub use ingest::{
    ASK_SCORE, ConceptError, ConceptExtractor, ConceptSummary, EXTRACTION_MODEL, IngestError,
    IngestSummary, Item, ItemCounts, LINK_SCORE, LoneImage, Models, SkippedItem, chapter_items,
    image_items, ingest_chapter, ingest_image,
};
pub use stores::Stores;
