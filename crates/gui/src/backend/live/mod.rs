//! The backend that reads the real stores and calls the real models, one module for each kind of
//! work the window asks for.

mod chapters;
mod context;
mod convert;
mod datasets;
mod dispatch;
mod failure;
pub mod ingest;
mod library;
pub mod query;
pub mod source;

pub use context::{Kept, LiveContext, RealServices, Services};
