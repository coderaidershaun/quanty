//! Turns a converted chapter into stored, searchable items and a graph of its document and
//! items, removes a document from both, and checks that the services this needs are ready.

mod delete;
pub mod health;
mod ingest;
mod items;
mod stores;

pub use delete::{DeleteError, DeleteSummary, delete_document};
pub use ingest::{IngestError, IngestSummary, ItemCounts, ingest_chapter};
pub use items::{Item, chapter_items};
pub use stores::Stores;
