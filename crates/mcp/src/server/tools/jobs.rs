//! The tools that start a job, follow it and stop it: check and ingest a PDF, write insights, build
//! a dataset, the status of each, and the tools that watch and list jobs of any kind.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::service::RequestContext;
use rmcp::{Json, RoleServer, tool, tool_router};

use crate::datasets::{BuildDatasetArgs, DatasetReport, DatasetStatusArgs};
use crate::ingest::{CheckPdfArgs, IngestPdfArgs, IngestReport, IngestStatusArgs, PdfCheck};
use crate::insights::{InsightsReport, InsightsStatusArgs, WriteInsightsArgs};
use crate::jobs::{BackgroundJobs, JobList, ProgressSink, WatchJobArgs};
use crate::jobs::{StopError, StopJobArgs, StopResult};
use crate::server::QuantyServer;
use crate::server::failure::ToolError;
use crate::server::lookup::{FoundJob, JobResult, JobStatusArgs, stop_any};
use crate::server::meta::{Cost, Group, Payer, ToolMarks, input_schema, output_schema};
use crate::server::tasks::JobStart;
use crate::services::Services;

/// How many jobs of each kind `list_jobs` gives.
const LISTED_PER_KIND: usize = 20;

#[tool_router(router = jobs_router, vis = "pub(in crate::server)")]
impl<S: Services> QuantyServer<S> {
    /// PDF check before `ingest_pdf`: title, pages, state, blockers, cost and time estimate. Free.
    #[tool(
        title = "Check PDF",
        annotations(read_only_hint = true, idempotent_hint = true),
        input_schema = input_schema::<CheckPdfArgs>(),
        output_schema = output_schema::<PdfCheck>(),
        meta = ToolMarks::new(Group::Ingest, Cost::Free).into()
    )]
    async fn check_pdf(
        &self,
        Parameters(args): Parameters<CheckPdfArgs>,
    ) -> Result<Json<PdfCheck>, ToolError> {
        Ok(Json(self.shared.ingest.check(args).await?))
    }

    /// Ingest, PAID (Ingest agent, Jev, Gemini): converts one PDF and stores it for search. Returns a `job_id`.
    #[tool(
        title = "Ingest PDF",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = true),
        input_schema = input_schema::<IngestPdfArgs>(),
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

    /// Deprecated: call `job_status`. Status of an ingest job.
    #[tool(
        title = "Ingest status",
        annotations(read_only_hint = true, idempotent_hint = true),
        input_schema = input_schema::<IngestStatusArgs>(),
        output_schema = output_schema::<IngestReport>(),
        meta = ToolMarks::new(Group::Jobs, Cost::Free).into()
    )]
    async fn ingest_status(
        &self,
        Parameters(args): Parameters<IngestStatusArgs>,
    ) -> Result<Json<IngestReport>, ToolError> {
        Ok(Json(self.shared.ingest.status(args)?))
    }

    /// Insights, PAID unless `dry_run`: writes missing insights, takeaways and cheat sheets. Returns a `job_id`.
    #[tool(
        title = "Write insights",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = true),
        input_schema = input_schema::<WriteInsightsArgs>(),
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

    /// Deprecated: call `job_status`. Status of a `write_insights` job.
    #[tool(
        title = "Insights status",
        annotations(read_only_hint = true, idempotent_hint = true),
        input_schema = input_schema::<InsightsStatusArgs>(),
        output_schema = output_schema::<InsightsReport>(),
        meta = ToolMarks::new(Group::Jobs, Cost::Free).into()
    )]
    async fn insights_status(
        &self,
        Parameters(args): Parameters<InsightsStatusArgs>,
    ) -> Result<Json<InsightsReport>, ToolError> {
        Ok(Json(self.shared.insights.status(args)?))
    }

    /// Dataset, PAID unless `dry_run`: builds a Gemma 4 training dataset from converted documents. Returns a `job_id`.
    #[tool(
        title = "Build dataset",
        annotations(read_only_hint = false, destructive_hint = true, idempotent_hint = true),
        input_schema = input_schema::<BuildDatasetArgs>(),
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

    /// Deprecated: call `job_status`. Status of a dataset build.
    #[tool(
        title = "Dataset status",
        annotations(read_only_hint = true, idempotent_hint = true),
        input_schema = input_schema::<DatasetStatusArgs>(),
        output_schema = output_schema::<DatasetReport>(),
        meta = ToolMarks::new(Group::Jobs, Cost::Free).into()
    )]
    async fn dataset_status(
        &self,
        Parameters(args): Parameters<DatasetStatusArgs>,
    ) -> Result<Json<DatasetReport>, ToolError> {
        Ok(Json(self.shared.datasets.status(args)?))
    }

    /// Stops an ingest, an insights job or a dataset build by `job_id`. What was paid for is kept.
    #[tool(
        title = "Stop job",
        annotations(read_only_hint = false, destructive_hint = true, idempotent_hint = true),
        input_schema = input_schema::<StopJobArgs>(),
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

    /// State, stage with counts and share done, usage and error of a job by `job_id`.
    #[tool(
        title = "Job status",
        annotations(read_only_hint = true, idempotent_hint = true),
        input_schema = input_schema::<JobStatusArgs>(),
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

    /// Waits up to `timeout_s` for a job to change, then gives its status. Sends progress.
    #[tool(
        title = "Watch job",
        annotations(read_only_hint = true, idempotent_hint = true),
        input_schema = input_schema::<WatchJobArgs>(),
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

    /// Jobs of this server, newest first, and the job that holds the shared slot.
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
