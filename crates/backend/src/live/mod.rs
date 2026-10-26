//! The backend that reads the real stores and calls the real models, one module for each kind of
//! work the window asks for.

mod balances;
mod chapter;
mod context;
mod convert;
mod datasets;
mod dispatch;
mod failure;
mod health;
pub mod ingest;
mod library;
pub mod query;

use std::path::Path;

pub use chapter::source;
pub use context::{Kept, LiveContext, RealServices, Services};
pub use failure::IntoFailure;

/// Stored picture paths become URLs relative to the one content folder the server serves.
pub(crate) fn image_url(path: &Path, root: &Path) -> Option<String> {
    rag_core::content_url(path, root).map(|url| format!("/{url}"))
}
