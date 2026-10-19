//! A Model Context Protocol server, so that an AI agent can search the stored books, read their
//! pages, send a chapter PDF to be ingested and build a training dataset from the converted
//! documents. `Services` gives it the paid services or stand-ins.

mod args;
mod datasets;
mod ingest;
mod jobs;
mod library;
mod retrieval;
mod server;
mod services;
mod transport;
mod usage;

pub use server::QuantyServer;
pub use services::{PaidServices, Services};
pub use transport::{ServeError, serve_http, serve_stdio};
