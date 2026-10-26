//! The tools that start a job, follow it and stop it: ingest a PDF, build a dataset, and the status
//! of each.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{Json, tool, tool_router};

use super::failure::ToolError;
use super::{QuantyServer, Shared};
use crate::datasets::{BuildDatasetArgs, DatasetReport, DatasetStatusArgs};
use crate::ingest::{IngestPdfArgs, IngestReport, IngestStatusArgs};
use crate::insights::{InsightsReport, InsightsStatusArgs, WriteInsightsArgs};
use crate::jobs::JobKind;
use crate::jobs::{StopError, StopJobArgs, StopResult, StopState};
use crate::services::Services;

#[tool_router(router = jobs_router, vis = "pub(super)")]
impl<S: Services> QuantyServer<S> {
    /// Converts one PDF of a book, a paper or another media and stores it so that it can be
    /// searched. It is PAID and slow: each page is read by the agent of the Ingest choice and Jev,
    /// the items are embedded by Gemini and that agent writes the chapter's insights, takeaways
    /// and cheat sheet and finds the concepts, which takes minutes for a chapter. The call
    /// answers when that work has started, with a `job_id` for `ingest_status`. A PDF that is
    /// already ingested costs nothing, unless its insights are not saved yet: then the agent
    /// writes those alone. A book's chapter must be named `chapter-<number>-<name>.pdf`; a part
    /// of a book that is not a chapter, such as its index, takes `document_title` and can have
    /// any file name; a paper or other PDF can have any name. Unified HTTP permits only one
    /// background ingest, insights job or dataset build across the app and MCP. Stdio permits
    /// one job of each kind at a time. The same PDF must not be sent through a second server,
    /// or beside `rag-ingest pdf`, at the same time. Sending the same PDF again goes on from where a
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

    /// Writes the missing insights, takeaways and cheat sheets of documents that are already
    /// stored, from their converted text, with nothing embedded. It is PAID unless `dry_run` is
    /// true: the agent of the Ingest choice is asked once for each document that has none, which
    /// spends its subscription usage and takes a minute or more for each. A dry run lists what
    /// would be written, asks nothing and costs nothing: run one first. Give exactly one of
    /// `document_id`, `media` and `all: true`. A document whose insights are saved is never asked
    /// about again, and a part of a book that is not a chapter gets none. The call answers once
    /// the work has begun, with a `job_id` for `insights_status`. Unified HTTP permits only one
    /// background job across the app and MCP. Stdio permits one job of each kind at a time.
    #[tool(annotations(
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true
    ))]
    async fn write_insights(
        &self,
        Parameters(args): Parameters<WriteInsightsArgs>,
    ) -> Result<Json<InsightsReport>, ToolError> {
        let error_text = |error| ToolError::Insights(error).text();
        Ok(Json(self.shared.insights.start(args, error_text).await?))
    }

    /// Says how a `write_insights` job is going, or how it ended: each document with what became
    /// of it, and what the questions used so far. Ask again every 20 to 30 seconds until the
    /// state is not `running`. Over stdio a job stops when the client closes the server.
    #[tool(annotations(read_only_hint = true))]
    async fn insights_status(
        &self,
        Parameters(args): Parameters<InsightsStatusArgs>,
    ) -> Result<Json<InsightsReport>, ToolError> {
        Ok(Json(self.shared.insights.status(args)?))
    }

    /// Builds a training dataset for Gemma 4 from the converted documents under the content
    /// folder: whole book sections for continued pre-training, rewritten texts and checked
    /// questions and answers for fine-tuning, and a manifest. It is PAID unless `dry_run` is true:
    /// the agent of the Datasets choice writes every rewritten text and answers every question,
    /// which spends its subscription usage and can take hours. A dry run exports, checks and
    /// plans, asks no model and costs nothing, and its summary says what the full build would
    /// ask: run one first. The call answers when the build has begun, with a `job_id` for
    /// `dataset_status`. Unified HTTP permits only one background job across the app and MCP.
    /// Stdio permits one job of each kind at a time. A build with the name of an earlier one
    /// replaces that dataset's files, and the answers a
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

    /// Stops a running job of this server: an ingest, an insights job or a dataset build, by the
    /// `job_id` that started it. What was paid for is kept: converted pages, kept answers and
    /// saved insights are not paid for again when the same work is started again. It costs
    /// nothing. A job that has ended already is left as it is.
    #[tool(annotations(
        read_only_hint = false,
        destructive_hint = true,
        idempotent_hint = true
    ))]
    async fn stop_job(
        &self,
        Parameters(args): Parameters<StopJobArgs>,
    ) -> Result<Json<StopResult>, ToolError> {
        let stopped = stop_any(&self.shared, &args.job_id).await;
        let (kind, state) = stopped.ok_or(StopError::UnknownJob {
            job_id: args.job_id.clone(),
        })?;
        Ok(Json(StopResult::new(&args.job_id, kind, state)))
    }
}

/// Stops the job with this id, whichever kind of job it is. `None` when no list holds it.
async fn stop_any<S: Services>(shared: &Shared<S>, job_id: &str) -> Option<(JobKind, StopState)> {
    if let Some(state) = shared.ingest.jobs.stop(job_id).await {
        return Some((JobKind::Ingest, state));
    }
    if let Some(state) = shared.insights.jobs.stop(job_id).await {
        return Some((JobKind::Insights, state));
    }
    shared
        .datasets
        .jobs
        .stop(job_id)
        .await
        .map(|state| (JobKind::Dataset, state))
}
