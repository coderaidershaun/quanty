//! A Model Context Protocol server, so that an AI agent can search the stored books, read their
//! pages and send a chapter PDF to be ingested. `Services` gives it the paid services or stand-ins.

mod ingest;
mod library;
mod retrieval;
mod server;
mod services;
mod transport;
mod usage;

pub use server::{DEFAULT_MAX_PDF_BYTES, QuantyServer};
pub use services::{PaidServices, Services};
pub use transport::{ServeError, serve_http, serve_stdio};
