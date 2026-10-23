//! The MCP server: one thin tool for each thing an agent can do, and the one place that words a
//! failure for the agent.

mod changes;
mod failure;
mod health;
mod jobs;
mod reading;

use std::sync::Arc;

use rag_core::Config;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::{ServerHandler, tool_handler};

use crate::datasets::Datasets;
use crate::ingest::{Ingest, base64_len};
use crate::insights::Insights;
use crate::jobs::BackgroundJobs;
use crate::services::Services;

/// The size a PDF may have unless the caller sets another limit: 50 MiB.
const DEFAULT_MAX_PDF_BYTES: u64 = 50 * 1024 * 1024;

/// Room in a request for everything but the base64 text of the PDF: 1 MiB.
const REQUEST_EXTRA_BYTES: u64 = 1024 * 1024;

/// The tools of quanty over its stored library. It is cheap to clone: a clone shares the
/// services and the jobs with the original.
pub struct QuantyServer<S: Services> {
    shared: Arc<Shared<S>>,
    tool_router: ToolRouter<Self>,
}

struct Shared<S> {
    config: Config,
    services: Arc<S>,
    ingest: Ingest<S>,
    insights: Insights<S>,
    datasets: Datasets<S>,
    max_pdf_bytes: u64,
}

impl<S: Services> Clone for QuantyServer<S> {
    fn clone(&self) -> Self {
        QuantyServer {
            shared: Arc::clone(&self.shared),
            tool_router: self.tool_router.clone(),
        }
    }
}

impl<S: Services> QuantyServer<S> {
    /// A server over the stores that `config` names, with the services of `services` and the
    /// default limit on the size of a PDF.
    pub fn new(config: Config, services: S) -> QuantyServer<S> {
        QuantyServer::with_max_pdf_bytes(config, services, DEFAULT_MAX_PDF_BYTES)
    }

    /// Like [`QuantyServer::new`], with a PDF of at most `max_pdf_bytes` bytes.
    pub fn with_max_pdf_bytes(config: Config, services: S, max_pdf_bytes: u64) -> QuantyServer<S> {
        Self::build(config, services, max_pdf_bytes, None)
    }

    /// Shares one background-work slot with the browser of the same HTTP server.
    pub fn with_background_jobs(
        config: Config,
        services: S,
        jobs: BackgroundJobs,
    ) -> QuantyServer<S> {
        Self::build(config, services, DEFAULT_MAX_PDF_BYTES, Some(jobs))
    }

    fn build(
        config: Config,
        services: S,
        max_pdf_bytes: u64,
        jobs: Option<BackgroundJobs>,
    ) -> QuantyServer<S> {
        let services = Arc::new(services);
        let ingest = Ingest::new(
            config.clone(),
            Arc::clone(&services),
            max_pdf_bytes,
            jobs.clone(),
        );
        let insights = Insights::new(config.clone(), Arc::clone(&services), jobs.clone());
        let datasets = Datasets::new(config.clone(), Arc::clone(&services), jobs);
        QuantyServer {
            shared: Arc::new(Shared {
                config,
                services,
                ingest,
                insights,
                datasets,
                max_pdf_bytes,
            }),
            tool_router: Self::reading_router() + Self::jobs_router() + Self::changes_router(),
        }
    }

    /// The largest request that can carry a PDF of the size limit as base64.
    pub(crate) fn largest_request_bytes(&self) -> u64 {
        base64_len(self.shared.max_pdf_bytes) + REQUEST_EXTRA_BYTES
    }
}

#[tool_handler(
    router = self.tool_router,
    name = "quanty",
    instructions = "Search and read a library of books, papers and other media. Start with `search` for a question, then `read_page` to read around a result. `chapter_contents`, `chapter_concepts` and `chapter_insights` give the items, the concepts and the saved notes of one document. `answer` writes a whole answer but spends the subscription usage of the agent of the Ask choice. `ingest_pdf` adds a PDF and `write_insights` writes missing notes, both paid; `build_dataset` builds a training dataset and is paid unless `dry_run` is true. Each starts a job that its status tool follows and `stop_job` stops. `edit_media` and `edit_document_tags` change labels and cost nothing. `export_insights` writes the saved notes of one document to a PDF, Markdown or HTML file, free."
)]
impl<S: Services> ServerHandler for QuantyServer<S> {}
