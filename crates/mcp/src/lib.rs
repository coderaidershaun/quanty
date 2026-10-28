//! Agents use the library through tools for reading and changing its contents.
//! Stand-in services let callers exercise the tools without paying for model calls.

mod args;
mod concepts;
mod datasets;
mod fixture;
mod ingest;
mod insights;
mod jobs;
mod library;
mod retrieval;
mod server;
mod services;
mod transport;
mod usage;

pub use fixture::fixture_http_service;
pub use jobs::{BackgroundJob, BackgroundJobs, BackgroundReservation, JobKind};
pub use server::QuantyServer;
pub use services::{PaidServices, Services};
pub use transport::{MCP_PATH, ServeError, http_service, serve_stdio};
