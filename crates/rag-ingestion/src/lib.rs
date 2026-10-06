//! Turns a converted chapter into stored, searchable items and a graph of its document, items and
//! the concepts they discuss, removes a document from both stores, and checks that the services
//! this needs are ready.

mod delete;
pub mod health;
mod ingest;
mod stores;

pub use delete::{DeleteError, DeleteSummary, delete_document};
pub use ingest::{
    ConceptError, ConceptExtractor, ConceptSummary, EXTRACTION_MODEL, IngestError, IngestSummary,
    Item, ItemCounts, Models, SkippedItem, chapter_items, ingest_chapter,
};
pub use stores::Stores;
