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

pub use chapter::source;
pub use context::{Kept, LiveContext, RealServices, Services};
pub use failure::IntoFailure;
