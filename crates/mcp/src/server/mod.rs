//! The MCP server: one thin tool for each thing an agent can do, and the one place that words a
//! failure for the agent.

mod catalog;
mod failure;
mod health;
mod lookup;
mod meta;
mod prompts;
mod resources;
mod tasks;
mod tools;

pub(crate) use meta::{Cost, Group, ToolMarks, output_schema};

use std::sync::Arc;

use rag_core::Config;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{
    CacheScope, CancelTaskParams, GetPromptRequestParams, GetPromptResponse, GetTaskParams,
    GetTaskResult, Implementation, ListPromptsResult, ListResourceTemplatesResult,
    ListResourcesResult, ListToolsResult, PaginatedRequestParams, ProtocolVersion,
    ReadResourceRequestParams, ReadResourceResponse, ServerCapabilities, ServerConfig,
    SubscriptionFilter, UpdateTaskParams,
};
use rmcp::service::{NotificationContext, RequestContext, SubscriptionContext};
use rmcp::{ErrorData, RoleServer, ServerHandler, tool_handler};

use crate::datasets::Datasets;
use crate::ingest::{Ingest, base64_len};
use crate::insights::Insights;
use crate::jobs::BackgroundJobs;
use crate::library::LibraryChanges;
use crate::services::Services;

/// The size a PDF may have unless the caller sets another limit: 50 MiB.
const DEFAULT_MAX_PDF_BYTES: u64 = 50 * 1024 * 1024;

/// Room in a request for everything but the base64 text of the PDF: 1 MiB.
const REQUEST_EXTRA_BYTES: u64 = 1024 * 1024;

/// How long a peer may keep the list of tools: five minutes. The list never changes while the
/// server runs and is the same for every caller.
const TOOL_LIST_TTL_MS: u64 = 300_000;

const INSTRUCTIONS: &str = "quanty serves a library of books, papers and other media: search it, read it by document, section and page, and walk its concepts.
Start with `health`, `catalog` and `document_overview`.
`catalog` lists the tools by group; `describe_tool` gives one in detail.
A tool that spends money has PAID as the second word of its description; check a PDF with `check_pdf` before `ingest_pdf`.
`guide` holds the rest: start, research, ingest, jobs, costs and errors.";

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
    /// The slot that the browser of the same HTTP server shares. `None` over stdio.
    background_jobs: Option<BackgroundJobs>,
    max_pdf_bytes: u64,
    /// Told when an ingest ends, and by the HTTP server that hosts this one when the browser
    /// changes the library, so that connected agents list the resources again.
    changes: LibraryChanges,
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
        Self::build(config, services, max_pdf_bytes, None, LibraryChanges::new())
    }

    /// Shares one background-work slot with the browser of the same HTTP server.
    pub fn with_background_jobs(
        config: Config,
        services: S,
        jobs: BackgroundJobs,
    ) -> QuantyServer<S> {
        Self::build(
            config,
            services,
            DEFAULT_MAX_PDF_BYTES,
            Some(jobs),
            LibraryChanges::new(),
        )
    }

    /// The server that an HTTP server hosts next to its browser: it shares the background-work
    /// slot with the browser, and `changes` is the place where the browser announces that it
    /// changed the library, so that connected agents list the resources again.
    pub fn hosted(
        config: Config,
        services: S,
        jobs: BackgroundJobs,
        changes: LibraryChanges,
    ) -> QuantyServer<S> {
        Self::build(config, services, DEFAULT_MAX_PDF_BYTES, Some(jobs), changes)
    }

    fn build(
        config: Config,
        services: S,
        max_pdf_bytes: u64,
        jobs: Option<BackgroundJobs>,
        changes: LibraryChanges,
    ) -> QuantyServer<S> {
        let services = Arc::new(services);
        let ingest = Ingest::new(
            config.clone(),
            Arc::clone(&services),
            max_pdf_bytes,
            jobs.clone(),
            changes.clone(),
        );
        let insights = Insights::new(config.clone(), Arc::clone(&services), jobs.clone());
        let datasets = Datasets::new(config.clone(), Arc::clone(&services), jobs.clone());
        QuantyServer {
            shared: Arc::new(Shared {
                config,
                services,
                ingest,
                insights,
                datasets,
                background_jobs: jobs,
                max_pdf_bytes,
                changes,
            }),
            tool_router: Self::reading_router()
                + Self::jobs_router()
                + Self::changes_router()
                + Self::library_router()
                + Self::concepts_router()
                + Self::catalog_router(),
        }
    }

    /// The largest request that can carry a PDF of the size limit as base64.
    pub(crate) fn largest_request_bytes(&self) -> u64 {
        base64_len(self.shared.max_pdf_bytes) + REQUEST_EXTRA_BYTES
    }
}

/// Only a peer of the 2026-07-28 protocol or later knows the cache hints of a list. rmcp does not
/// remove them from the reply to an older peer, so the server must not add them.
fn sends_cache_hints(context: &RequestContext<RoleServer>) -> bool {
    context
        .protocol_version()
        .is_some_and(|version| version >= ProtocolVersion::V_2026_07_28)
}

#[tool_handler(router = self.tool_router)]
impl<S: Services> ServerHandler for QuantyServer<S> {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_tasks()
                .enable_resources()
                .enable_resources_list_changed()
                .enable_prompts()
                .build(),
        )
        .with_server_info(Implementation::new("quanty", env!("CARGO_PKG_VERSION")))
        .with_instructions(INSTRUCTIONS)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let tools =
            ListToolsResult::with_all_items(catalog::in_list_order(self.tool_router.list_all()));
        if sends_cache_hints(&context) {
            Ok(tools
                .with_ttl_ms(TOOL_LIST_TTL_MS)
                .with_cache_scope(CacheScope::Public))
        } else {
            Ok(tools)
        }
    }

    async fn get_task(
        &self,
        request: GetTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetTaskResult, ErrorData> {
        tasks::get(&self.shared, &request.task_id)
    }

    async fn update_task(
        &self,
        request: UpdateTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        tasks::update(&self.shared, &request.task_id)
    }

    async fn cancel_task(
        &self,
        request: CancelTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        tasks::cancel(&self.shared, &request.task_id).await
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, ErrorData> {
        Ok(resources::templates(sends_cache_hints(&context)))
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        resources::list(&self.shared.config, sends_cache_hints(&context)).await
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        let hints = sends_cache_hints(&context);
        let read = resources::read(&self.shared.config, &request.uri, hints).await?;
        Ok(read.into())
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, ErrorData> {
        Ok(prompts::list(sends_cache_hints(&context)))
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResponse, ErrorData> {
        Ok(prompts::get(request)?.into())
    }

    fn accepted_subscription_filter(
        &self,
        _requested: &SubscriptionFilter,
    ) -> Option<SubscriptionFilter> {
        Some(resources::accepted_filter())
    }

    async fn listen(&self, subscription: SubscriptionContext) -> Result<(), ErrorData> {
        resources::listen(&self.shared.changes, subscription).await
    }

    async fn on_initialized(&self, context: NotificationContext<RoleServer>) {
        resources::tell_session(&self.shared.changes, context.peer);
    }
}
