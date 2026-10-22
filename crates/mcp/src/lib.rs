//! A Model Context Protocol server, so that an AI agent can search the stored books, read their
//! pages and the items, concepts and insights of a chapter, send a chapter PDF to be ingested,
//! write missing insights, export them to a file, change labels, stop a job and build a training
//! dataset from the converted documents. `Services` gives it the paid services or stand-ins.

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

pub use server::QuantyServer;
pub use services::{PaidServices, Services};
pub use transport::{ServeError, serve_http, serve_stdio};
