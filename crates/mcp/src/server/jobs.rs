//! The tools that start a job, follow it and stop it: ingest a PDF, write insights, build a
//! dataset, the status of each, and the tools that watch and list jobs of any kind.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::service::RequestContext;
use rmcp::{Json, RoleServer, tool, tool_router};

use super::QuantyServer;
use super::failure::ToolError;
use super::lookup::{FoundJob, JobResult, JobStatusArgs, stop_any};
use super::meta::{Cost, Group, Payer, ToolMarks, output_schema};
use super::tasks::JobStart;
use crate::datasets::{BuildDatasetArgs, DatasetReport, DatasetStatusArgs};
use crate::ingest::{IngestPdfArgs, IngestReport, IngestStatusArgs};
use crate::insights::{InsightsReport, InsightsStatusArgs, WriteInsightsArgs};
use crate::jobs::{BackgroundJobs, JobList, ProgressSink, WatchJobArgs};
use crate::jobs::{StopError, StopJobArgs, StopResult};
use crate::services::Services;

/// How many jobs of each kind `list_jobs` gives.
const LISTED_PER_KIND: usize = 20;

#[tool_router(router = jobs_router, vis = "pub(super)")]
impl<S: Services> QuantyServer<S> {
    /// Converts one PDF of a book, a paper or another media and stores it so that it can be
    /// searched. It is PAID and slow: each page is read by the agent of the Ingest choice and Jev,
    /// the items are embedded by Gemini and that agent writes the chapter's insights, takeaways
    /// and cheat sheet and finds the concepts, which takes minutes for a chapter. The call
    /// answers when that work has started, with a `job_id`: call `watch_job` with it (it waits for
    /// you) or `job_status`. A PDF that is already ingested costs nothing, unless its insights are
    /// not saved yet: then the agent writes those alone. A book's chapter must be named
    /// `chapter-<number>-<name>.pdf`; a part of a book that is not a chapter, such as its index,
    /// takes `document_title` and can have any file name; a paper or other PDF can have any name.
    /// Unified HTTP permits only one background ingest, insights job or dataset build across the
    /// app and MCP. Stdio permits one job of each kind at a time. The same PDF must not be sent
    /// through a second server, or beside `rag-ingest pdf`, at the same time. Sending the same PDF
    /// again goes on from where a stopped run ended.
    #[tool(
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true
        ),
        output_schema = output_schema::<IngestReport>(),
        meta = ToolMarks::new(Group::Ingest, Cost::Paid(Payer::Ingest))
            .with_job("job_status", "job_id")
            .into()
    )]
    async fn ingest_pdf(
        &self,
        Parameters(args): Parameters<IngestPdfArgs>,
        context: RequestContext<RoleServer>,
    ) -> Result<JobStart<IngestReport>, ToolError> {
        let error_text = |error| ToolError::Ingest(error).text();
        let report = self.shared.ingest.start(args, error_text).await?;
        Ok(JobStart::new(report, &context))
    }

    /// Deprecated: call `job_status`. Says how an ingest job is going, or how it ended. Over stdio
    /// a job stops when the client closes the server.
    #[tool(
        annotations(read_only_hint = true),
        output_schema = output_schema::<IngestReport>(),
        meta = ToolMarks::new(Group::Jobs, Cost::Free).into()
    )]
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
    /// the work has begun, with a `job_id`: call `watch_job` with it (it waits for you) or
    /// `job_status`. Unified HTTP permits only one background job across the app and MCP. Stdio
    /// permits one job of each kind at a time.
    #[tool(
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true
        ),
        output_schema = output_schema::<InsightsReport>(),
        meta = ToolMarks::new(Group::Ingest, Cost::Paid(Payer::Ingest))
            .with_job("job_status", "job_id")
            .into()
    )]
    async fn write_insights(
        &self,
        Parameters(args): Parameters<WriteInsightsArgs>,
        context: RequestContext<RoleServer>,
    ) -> Result<JobStart<InsightsReport>, ToolError> {
        let error_text = |error| ToolError::Insights(error).text();
        let report = self.shared.insights.start(args, error_text).await?;
        Ok(JobStart::new(report, &context))
    }

    /// Deprecated: call `job_status`. Says how a `write_insights` job is going, or how it ended:
    /// each document with what became of it, and what the questions used so far. Over stdio a job
    /// stops when the client closes the server.
    #[tool(
        annotations(read_only_hint = true),
        output_schema = output_schema::<InsightsReport>(),
        meta = ToolMarks::new(Group::Jobs, Cost::Free).into()
    )]
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
    /// ask: run one first. The call answers when the build has begun, with a `job_id`: call
    /// `watch_job` with it (it waits for you) or `job_status`. Unified HTTP permits only one
    /// background job across the app and MCP. Stdio permits one job of each kind at a time. A
    /// build with the name of an earlier one replaces that dataset's files, and the answers a
    /// model already gave are kept and not paid for twice.
    #[tool(
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true
        ),
        output_schema = output_schema::<DatasetReport>(),
        meta = ToolMarks::new(Group::Datasets, Cost::Paid(Payer::Datasets))
            .with_job("job_status", "job_id")
            .into()
    )]
    async fn build_dataset(
        &self,
        Parameters(args): Parameters<BuildDatasetArgs>,
        context: RequestContext<RoleServer>,
    ) -> Result<JobStart<DatasetReport>, ToolError> {
        let error_text = |error| ToolError::Dataset(error).text();
        let report = self.shared.datasets.start(args, error_text).await?;
        Ok(JobStart::new(report, &context))
    }

    /// Deprecated: call `job_status`. Says how a dataset build is going, or how it ended, with
    /// what its models spent so far. Over stdio a build stops when the client closes the server.
    #[tool(
        annotations(read_only_hint = true),
        output_schema = output_schema::<DatasetReport>(),
        meta = ToolMarks::new(Group::Jobs, Cost::Free).into()
    )]
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
    #[tool(
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true
        ),
        output_schema = output_schema::<StopResult>(),
        meta = ToolMarks::new(Group::Jobs, Cost::Free).into()
    )]
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

    /// Job status of an ingest, an insights job or a dataset build by its `job_id`: state, stage
    /// with counts and share done, usage and error. Free.
    #[tool(
        title = "Job status",
        annotations(read_only_hint = true, idempotent_hint = true),
        output_schema = output_schema::<JobResult>(),
        meta = ToolMarks::new(Group::Jobs, Cost::Free)
            .with_job("job_status", "job_id")
            .into()
    )]
    async fn job_status(
        &self,
        Parameters(args): Parameters<JobStatusArgs>,
    ) -> Result<Json<JobResult>, ToolError> {
        let found = FoundJob::find(&self.shared, &args.job_id)?;
        Ok(Json(found.result()))
    }

    /// Job watch: waits up to `timeout_s` seconds (default 60, at most 90) until the stage or
    /// state of a job changes, then gives what `job_status` gives. With a progress token it sends
    /// progress notifications while it waits. Free.
    #[tool(
        title = "Watch job",
        annotations(read_only_hint = true, idempotent_hint = true),
        output_schema = output_schema::<JobResult>(),
        meta = ToolMarks::new(Group::Jobs, Cost::Free)
            .with_job("job_status", "job_id")
            .into()
    )]
    async fn watch_job(
        &self,
        Parameters(args): Parameters<WatchJobArgs>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<JobResult>, ToolError> {
        let timeout = args.timeout()?;
        let found = FoundJob::find(&self.shared, &args.job_id)?;
        let progress = context
            .meta
            .get_progress_token()
            .map(|token| ProgressSink::new(context.peer.clone(), token));
        let result = found
            .wait_for_change(timeout, progress, context.ct.cancelled())
            .await;
        Ok(Json(result))
    }

    /// Jobs of this server, newest first, and the job that holds the shared background slot.
    /// Free.
    #[tool(
        title = "List jobs",
        annotations(read_only_hint = true, idempotent_hint = true),
        output_schema = output_schema::<JobList>(),
        meta = ToolMarks::new(Group::Jobs, Cost::Free).into()
    )]
    async fn list_jobs(&self) -> Json<JobList> {
        let shared = &self.shared;
        let mut jobs = shared.ingest.jobs.newest(LISTED_PER_KIND);
        jobs.extend(shared.insights.jobs.newest(LISTED_PER_KIND));
        jobs.extend(shared.datasets.jobs.newest(LISTED_PER_KIND));
        let running = shared
            .background_jobs
            .as_ref()
            .and_then(BackgroundJobs::running);
        Json(JobList::new(running, jobs))
    }
}
