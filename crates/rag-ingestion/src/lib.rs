//! Turns a converted chapter into stored, searchable items, and checks that the services this
//! needs are ready.

pub mod health;
mod ingest;
mod items;

pub use ingest::{IngestError, IngestSummary, ItemCounts, ingest_chapter};
pub use items::{Item, chapter_items};
