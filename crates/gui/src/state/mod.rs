//! The state that every panel reads, and the rules that change it when a person acts or the
//! backend answers. It knows nothing of the window.

// SMELL: `Shared` holds the type of each file here, and each file adds its rules to `Shared`, so
// they all import each other. Separating them means moving every rule onto its own type.
mod ask;
mod health;
mod ingest;
mod library;
mod shared;
mod source;

pub use ask::AskSession;
pub use health::{Health, HealthLevel};
pub use ingest::IngestJob;
pub use library::{Busy, Library, MediaEditing, MediaSave};
pub use shared::{Cues, Quit, Shared};
pub use source::{SourceNav, SourceTarget};
