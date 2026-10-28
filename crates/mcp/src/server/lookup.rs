//! Finds a job of any kind by its id, and gives it as the `Job` every kind shares or as the whole
//! result of its kind, so that every tool that is about a job by its id reads it the same way.

use std::borrow::Cow;
use std::future::Future;
use std::time::Duration;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use super::Shared;
use crate::datasets::DatasetReport;
use crate::ingest::IngestReport;
use crate::insights::InsightsReport;
use crate::jobs::{Job, JobReport, ProgressSink, StopError, wait_for_change};
use crate::services::Services;

/// What an agent gives `job_status`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct JobStatusArgs {
    /// The `job_id` that `ingest_pdf`, `write_insights` or `build_dataset` gave.
    pub(crate) job_id: String,
}

/// The whole result of a job: the fields of the `Job` that every kind has, and the fields of its
/// own kind beside them.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum JobResult {
    Ingest(IngestReport),
    Insights(InsightsReport),
    Dataset(DatasetReport),
}

/// The schema is one object, because a client wants an object at the root of a result schema. The
/// keys of `Job` are its properties, and the fields of each kind are the three choices of `anyOf`.
impl JsonSchema for JobResult {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("JobResult")
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let job = Job::json_schema(generator);
        let job_fields = job.as_object();
        let kinds = [
            generator.subschema_for::<IngestReport>(),
            generator.subschema_for::<InsightsReport>(),
            generator.subschema_for::<DatasetReport>(),
        ];
        json_schema!({
            "type": "object",
            "properties": job_fields.and_then(|fields| fields.get("properties")).cloned(),
            "required": job_fields.and_then(|fields| fields.get("required")).cloned(),
            "anyOf": kinds,
        })
    }
}

/// A job of this server, looked up by its id.
pub(crate) enum FoundJob {
    Ingest(watch::Receiver<IngestReport>),
    Insights(watch::Receiver<InsightsReport>),
    Dataset(watch::Receiver<DatasetReport>),
}

impl FoundJob {
    /// The job with this id, whichever kind it is.
    ///
    /// # Errors
    /// [`StopError::UnknownJob`] when no list holds the id.
    pub(crate) fn find<S: Services>(
        shared: &Shared<S>,
        job_id: &str,
    ) -> Result<FoundJob, StopError> {
        if let Some(watcher) = shared.ingest.jobs.watcher_of(job_id) {
            return Ok(FoundJob::Ingest(watcher));
        }
        if let Some(watcher) = shared.insights.jobs.watcher_of(job_id) {
            return Ok(FoundJob::Insights(watcher));
        }
        shared
            .datasets
            .jobs
            .watcher_of(job_id)
            .map(FoundJob::Dataset)
            .ok_or_else(|| StopError::UnknownJob {
                job_id: job_id.to_owned(),
            })
    }

    /// What the job is now, as every kind of job tells it, without the fields of its kind.
    #[expect(
        dead_code,
        reason = "no tool reads a job without the fields of its kind yet"
    )]
    pub(crate) fn job(&self) -> Job {
        match self {
            FoundJob::Ingest(watcher) => watcher.borrow().job().clone(),
            FoundJob::Insights(watcher) => watcher.borrow().job().clone(),
            FoundJob::Dataset(watcher) => watcher.borrow().job().clone(),
        }
    }

    /// The whole result of the job now.
    pub(crate) fn result(&self) -> JobResult {
        match self {
            FoundJob::Ingest(watcher) => JobResult::Ingest(watcher.borrow().clone()),
            FoundJob::Insights(watcher) => JobResult::Insights(watcher.borrow().clone()),
            FoundJob::Dataset(watcher) => JobResult::Dataset(watcher.borrow().clone()),
        }
    }

    /// Waits as [`wait_for_change`] does, and gives the whole result of the job then.
    pub(crate) async fn wait_for_change(
        self,
        timeout: Duration,
        progress: Option<ProgressSink>,
        cancelled: impl Future<Output = ()>,
    ) -> JobResult {
        match self {
            FoundJob::Ingest(watcher) => {
                JobResult::Ingest(wait_for_change(watcher, timeout, progress, cancelled).await)
            }
            FoundJob::Insights(watcher) => {
                JobResult::Insights(wait_for_change(watcher, timeout, progress, cancelled).await)
            }
            FoundJob::Dataset(watcher) => {
                JobResult::Dataset(wait_for_change(watcher, timeout, progress, cancelled).await)
            }
        }
    }
}
