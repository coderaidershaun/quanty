//! The state that every panel reads, and the rules that change it when a person acts or the
//! backend answers. It knows nothing of the window.

// SMELL: `Shared` holds the types that the other files of this folder define, and each of those
// files adds its rules to `Shared`. So `Shared` and every one of them import each other, and a
// rule cannot move without the type beside it.
mod ask;
mod health;
mod ingest;
mod library;
mod shared;
mod source;

pub use ask::AskSession;
pub use health::{Health, HealthLevel};
pub use ingest::IngestJob;
pub use library::{Busy, Library};
pub use shared::{Cues, Quit, Shared};
pub use source::{SourceNav, SourceTarget};
