//! The MCP server: one thin tool for each thing an agent can do, and the one place that words a
//! failure for the agent.

use std::sync::Arc;

use ocr::{ConvertError, PageError};
use rag_core::{Config, error_chain};
use rag_ingestion::{ConceptError, IngestError, PdfError, health};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::tool::IntoCallToolResult;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResponse, CallToolResult, ContentBlock};
use rmcp::{ErrorData, Json, ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::Serialize;

use crate::datasets::{
    BuildDatasetArgs, DatasetReport, DatasetStatusArgs, DatasetToolError, Datasets,
};
use crate::ingest::{
    Ingest, IngestPdfArgs, IngestReport, IngestStatusArgs, PdfIngestError, base64_len,
};
use crate::library::{self, Documents, LibraryError, PageView, ReadPageArgs};
use crate::retrieval::{
    self, AnswerResult, QuestionArgs, RetrievalError, SearchArgs, SearchResult,
};
use crate::services::Services;

/// The size a PDF may have unless the caller sets another limit: 50 MiB.
const DEFAULT_MAX_PDF_BYTES: u64 = 50 * 1024 * 1024;

/// Room in a request for everything but the base64 text of the PDF: 1 MiB.
const REQUEST_EXTRA_BYTES: u64 = 1024 * 1024;

/// Added to the text of a failure of a store or of an outside service.
const HEALTH_HINT: &str = "Call the `health` tool to see which service is not ready.";

/// The tools of quanty over its stored library. It is cheap to clone: a clone shares the
/// services, the ingest jobs and the dataset builds with the original.
pub struct QuantyServer<S: Services> {
    shared: Arc<Shared<S>>,
    tool_router: ToolRouter<Self>,
}

struct Shared<S> {
    config: Config,
    services: Arc<S>,
    ingest: Ingest<S>,
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

/// Whether the services that a tool needs are ready.
#[derive(Serialize, JsonSchema)]
struct HealthResult {
    /// True when Qdrant, FalkorDB and the sign-in of the Ingest choice's agent are all ready.
    healthy: bool,
    /// One line for each service, saying whether it is ready.
    report: String,
}

/// Why a tool call failed. It reaches the agent as a tool error, with a text that says what to do.
#[derive(thiserror::Error, Debug)]
enum ToolError {
    #[error(transparent)]
    Retrieval(#[from] RetrievalError),

    #[error(transparent)]
    Library(#[from] LibraryError),

    #[error(transparent)]
    Ingest(#[from] PdfIngestError),

    #[error(transparent)]
    Dataset(#[from] DatasetToolError),
}

impl ToolError {
    /// The text of the failure: the error and every cause under it on one line. A failure of a
    /// store or of an outside service points at the `health` tool.
    fn text(&self) -> String {
        let mut text = error_chain(self)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if !text.ends_with(['.', '!', '?']) {
            text.push('.');
        }
        if self.is_service_failure() {
            text.push(' ');
            text.push_str(HEALTH_HINT);
        }
        text
    }

    fn is_service_failure(&self) -> bool {
        match self {
            ToolError::Retrieval(error) => matches!(
                error,
                RetrievalError::Graph(_)
                    | RetrievalError::Embedder(_)
                    | RetrievalError::ItemStore(_)
                    | RetrievalError::ConceptStore(_)
                    | RetrievalError::Search(_)
                    | RetrievalError::Answer(rag_retrieval::AnswerError::Llm(_))
            ),
            ToolError::Library(error) => matches!(error, LibraryError::Graph(_)),
            // A conversion also fails for the PDF itself, or for a reply that broke a rule twice,
            // and no service is the cause then. It points at a service only when the services
            // could not start or a call for a page failed.
            ToolError::Ingest(PdfIngestError::Pdf(error)) => match &**error {
                PdfError::Convert(ConvertError::Services(_)) => true,
                PdfError::Convert(ConvertError::PageFailed { source, .. }) => {
                    matches!(**source, PageError::Service(_))
                }
                PdfError::Convert(_) => false,
                PdfError::Graph(_) | PdfError::Store(_) => true,
                PdfError::Ingest(error) => is_service_failure_of_the_ingest(error),
            },
            ToolError::Ingest(error) => matches!(
                error,
                PdfIngestError::Embedder(_)
                    | PdfIngestError::ItemStore(_)
                    | PdfIngestError::ConceptStore(_)
                    | PdfIngestError::Graph(_)
                    | PdfIngestError::ReadMedia(_)
                    | PdfIngestError::Labels(_)
            ),
            ToolError::Dataset(error) => match error {
                DatasetToolError::Graph(_) | DatasetToolError::ModelNotReady(_) => true,
                // A build that the model or the graph stopped points at `health`. A file, a folder
                // or the content folder is not a service, and `health` would call every service
                // ready.
                DatasetToolError::Build(error) => {
                    error.llm_error().is_some() || error.graph_error().is_some()
                }
                // None of these is a service. The text of a job that failed already got its hint,
                // if it needed one, when the job ended.
                DatasetToolError::BlankName
                | DatasetToolError::Name { .. }
                | DatasetToolError::UnknownMedia { .. }
                | DatasetToolError::UnknownDocument { .. }
                | DatasetToolError::UnknownHoldout { .. }
                | DatasetToolError::NothingToExport
                | DatasetToolError::Busy { .. }
                | DatasetToolError::UnknownJob { .. }
                | DatasetToolError::Lost
                | DatasetToolError::Failed(_)
                | DatasetToolError::Stopped(_) => false,
            },
        }
    }
}

/// Whether a store or an outside service stopped the ingest of a converted chapter. A file that
/// cannot be read or written, a reply that the model got wrong twice and stores that do not belong
/// together are not that, and `health` would call every service ready.
fn is_service_failure_of_the_ingest(error: &IngestError) -> bool {
    match error {
        IngestError::ModelNotReady(_)
        | IngestError::Embed(_)
        | IngestError::Store(_)
        | IngestError::Graph(_) => true,
        IngestError::Concepts(error) => matches!(
            error,
            ConceptError::Stopped { .. }
                | ConceptError::Embed { .. }
                | ConceptError::ConceptStore(_)
                | ConceptError::RelatedItems { .. }
                | ConceptError::Graph(_)
        ),
        _ => false,
    }
}

/// A tool error is a result with `isError` set and a text, never a protocol error: a client shows
/// a protocol error without its message.
impl IntoCallToolResult for ToolError {
    fn into_call_tool_result(self) -> Result<CallToolResponse, ErrorData> {
        Ok(CallToolResult::error(vec![ContentBlock::text(self.text())]).into())
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
        let services = Arc::new(services);
        let ingest = Ingest::new(config.clone(), Arc::clone(&services), max_pdf_bytes);
        let datasets = Datasets::new(config.clone(), Arc::clone(&services));
        QuantyServer {
            shared: Arc::new(Shared {
                config,
                services,
                ingest,
                datasets,
                max_pdf_bytes,
            }),
            tool_router: Self::tool_router(),
        }
    }

    /// The largest request that can carry a PDF of the size limit as base64.
    pub(crate) fn largest_request_bytes(&self) -> u64 {
        base64_len(self.shared.max_pdf_bytes) + REQUEST_EXTRA_BYTES
    }
}

#[tool_router]
impl<S: Services> QuantyServer<S> {
    /// Finds the stored items (text chunks, formulas, figures, tables and code listings) that
    /// are nearest to a question, with their document, page and the reason each was found. It
    /// embeds the question with the Gemini API, which is a small paid call. Give the
    /// `document_id` and `page` of a result to `read_page` to read the page around it. Give
    /// `document` (a `title` or `document_id` from `list_documents`) to search one document only.
    #[tool(annotations(read_only_hint = true))]
    async fn search(
        &self,
        Parameters(args): Parameters<SearchArgs>,
    ) -> Result<Json<SearchResult>, ToolError> {
        let shared = &self.shared;
        Ok(Json(
            retrieval::search(&shared.config, &*shared.services, args).await?,
        ))
    }

    /// Writes an answer to a question from what `search` finds, with the items that each
    /// statement rests on. It uses the subscription of the agent of the Ask choice and embeds the
    /// question with Gemini. It can take a minute or two. An agent that can write its own answer
    /// should call `search` instead.
    #[tool(annotations(read_only_hint = true))]
    async fn answer(
        &self,
        Parameters(args): Parameters<QuestionArgs>,
    ) -> Result<Json<AnswerResult>, ToolError> {
        let shared = &self.shared;
        Ok(Json(
            retrieval::answer(&shared.config, &*shared.services, args).await?,
        ))
    }

    /// Lists every stored document with its media, category, authors, tags and chapter. Give the
    /// `document_id` of one to `read_page`, and its `title` or `document_id` as `document` to
    /// `search` or `answer` to ask that document alone.
    #[tool(annotations(read_only_hint = true))]
    async fn list_documents(&self) -> Result<Json<Documents>, ToolError> {
        Ok(Json(library::list_documents(&self.shared.config).await?))
    }

    /// Reads the pieces of one page of a chapter in reading order, to read around a search result.
    /// It reads the saved chapter and needs no store.
    #[tool(annotations(read_only_hint = true))]
    async fn read_page(
        &self,
        Parameters(args): Parameters<ReadPageArgs>,
    ) -> Result<Json<PageView>, ToolError> {
        Ok(Json(library::read_page(&self.shared.config, args)?))
    }

    /// Says whether Qdrant, FalkorDB and the sign-in of the Ingest choice's agent are ready. It
    /// costs nothing.
    #[tool(annotations(read_only_hint = true))]
    async fn health(&self) -> Json<HealthResult> {
        let report = health::check(&self.shared.config).await;
        Json(HealthResult {
            healthy: report.is_healthy(),
            report: report.to_string(),
        })
    }

    /// Converts one PDF of a book, a paper or another media and stores it so that it can be
    /// searched. It is PAID and slow: each page is read by the agent of the Ingest choice and Jev,
    /// the items are embedded by Gemini and that agent writes the chapter's insights, takeaways
    /// and cheat sheet and finds the concepts, which takes minutes for a chapter. The call
    /// answers when that work has started, with a `job_id` for `ingest_status`. A PDF that is
    /// already ingested costs nothing, unless its insights are not saved yet: then the agent
    /// writes those alone. A book's chapter must be named `chapter-<number>-<name>.pdf`; a part
    /// of a book that is not a chapter, such as its index, takes `document_title` and can have
    /// any file name; a paper or other PDF can have any name. One ingest runs at a time in this
    /// server, and the same PDF must not be sent through a second server, or beside
    /// `rag-ingest pdf`, at the same time. Sending the same PDF again goes on from where a
    /// stopped run ended.
    #[tool(annotations(
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true
    ))]
    async fn ingest_pdf(
        &self,
        Parameters(args): Parameters<IngestPdfArgs>,
    ) -> Result<Json<IngestReport>, ToolError> {
        let error_text = |error| ToolError::Ingest(error).text();
        Ok(Json(self.shared.ingest.start(args, error_text).await?))
    }

    /// Says how an ingest job is going, or how it ended. Ask again every 20 to 30 seconds until
    /// the state is not `running`. Over stdio a job stops when the client closes the server.
    #[tool(annotations(read_only_hint = true))]
    async fn ingest_status(
        &self,
        Parameters(args): Parameters<IngestStatusArgs>,
    ) -> Result<Json<IngestReport>, ToolError> {
        Ok(Json(self.shared.ingest.status(args)?))
    }

    /// Builds a training dataset for Gemma 4 from the converted documents under the content
    /// folder: whole book sections for continued pre-training, rewritten texts and checked
    /// questions and answers for fine-tuning, and a manifest. It is PAID unless `dry_run` is true:
    /// the agent of the Datasets choice writes every rewritten text and answers every question,
    /// which spends its subscription usage and can take hours. A dry run exports, checks and
    /// plans, asks no model and costs nothing, and its summary says what the full build would
    /// ask: run one first. The call answers when the build has begun, with a `job_id` for
    /// `dataset_status`. One build runs at a time in this server; an ingest can run beside it. A
    /// build with the name of an earlier one replaces that dataset's files, and the answers a
    /// model already gave are kept and not paid for twice.
    #[tool(annotations(
        read_only_hint = false,
        destructive_hint = true,
        idempotent_hint = true
    ))]
    async fn build_dataset(
        &self,
        Parameters(args): Parameters<BuildDatasetArgs>,
    ) -> Result<Json<DatasetReport>, ToolError> {
        let error_text = |error| ToolError::Dataset(error).text();
        Ok(Json(self.shared.datasets.start(args, error_text).await?))
    }

    /// Says how a dataset build is going, or how it ended, with what its models spent so far. Ask
    /// again every 20 to 30 seconds until the state is not `running`. Over stdio a build stops when
    /// the client closes the server.
    #[tool(annotations(read_only_hint = true))]
    async fn dataset_status(
        &self,
        Parameters(args): Parameters<DatasetStatusArgs>,
    ) -> Result<Json<DatasetReport>, ToolError> {
        Ok(Json(self.shared.datasets.status(args)?))
    }
}

#[tool_handler(
    router = self.tool_router,
    name = "quanty",
    instructions = "Search and read a library of books, papers and other media. Start with `search` for a question, then `read_page` to read around a result. `answer` writes a whole answer but spends the subscription usage of the agent of the Ask choice. `ingest_pdf` adds a PDF, and `ingest_status` follows it. `build_dataset` builds a training dataset from the converted documents and spends the subscription usage of the agent of the Datasets choice unless `dry_run` is true; `dataset_status` follows it."
)]
impl<S: Services> ServerHandler for QuantyServer<S> {}
