//! Agents use the library through tools for reading and changing its contents.
//! Stand-in services let callers exercise the tools without paying for model calls.

mod args;
mod datasets;
mod exporting;
mod ingest;
mod insights;
mod jobs;
mod library;
mod retrieval;
mod server;
mod services;
mod transport;
mod usage;

pub use jobs::{BackgroundJob, BackgroundJobs, BackgroundReservation, JobKind};
pub use server::QuantyServer;
pub use services::{PaidServices, Services};
pub use transport::{ServeError, http_service, serve_stdio};
