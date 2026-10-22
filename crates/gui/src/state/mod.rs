//! The state that every panel reads, and the rules that change it when a person acts or the
//! backend answers. It knows nothing of the window.

// SMELL: `Shared` holds the type of each file here, and each file adds its rules to `Shared`, so
// they all import each other. Separating them means moving every rule onto its own type.
mod ask;
mod balances;
mod contents;
mod datasets;
mod exporting;
mod health;
mod ingest;
mod insights;
mod library;
mod shared;
mod shelf;
mod source;

pub use ask::AskSession;
pub use balances::Balances;
pub use contents::Contents;
pub use datasets::DatasetJob;
pub use exporting::{ExportJob, ExportState};
pub use health::{DatabaseLink, Health, HealthLevel};
pub use ingest::IngestJob;
pub use insights::Insights;
pub use library::{Busy, Library, MediaDelete, MediaEditing, MediaSave};
pub use shared::{Cues, Quit, Shared};
pub use shelf::{Open, Shelf};
pub use source::{SourceNav, SourceTarget};
