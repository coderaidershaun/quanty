//! A Model Context Protocol server, so that an AI agent can search the stored books, read their
//! pages and send a chapter PDF to be ingested. `QuantyServer` holds the tools, `serve_stdio` and
//! `serve_http` carry them to a client, and `Services` is how a caller gives it the paid
//! services, or stand-ins in a test.

mod ingest;
mod library;
mod retrieval;
mod server;
mod services;
mod transport;

pub use server::{DEFAULT_MAX_PDF_BYTES, QuantyServer};
pub use services::{PaidServices, Services};
pub use transport::{ServeError, serve_http, serve_stdio};
