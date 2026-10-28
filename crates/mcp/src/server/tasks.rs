//! The tasks extension over the job lists: a client that declares it follows a job as a task whose
//! id is the id of the job, so that it can poll, wait and cancel without knowing the job tools.

use rmcp::handler::server::tool::IntoCallToolResult;
use rmcp::handler::server::wrapper::Json;
use rmcp::model::{
    CallToolResponse, CallToolResult, CreateTaskResult, DetailedTask, GetTaskResult, Task,
    TaskPayload,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use super::Shared;
use super::lookup::{FoundJob, stop_any};
use crate::jobs::{Job, JobReport, JobState, StopError, rfc3339};
use crate::services::Services;

const POLL_INTERVAL_MS: u64 = 5_000;

/// The job lists can forget a job before a day has passed, once more than their limit of newer
/// jobs have been started; `tasks/get` then says that the id is unknown.
const TASK_TTL_MS: u64 = 86_400_000;

/// What a tool that starts a job answers: the report of the job, or a task for the job when the
/// client declared the tasks extension.
pub(super) struct JobStart<R> {
    report: R,
    client_declared_tasks: bool,
}

impl<R> JobStart<R> {
    pub(super) fn new(report: R, context: &RequestContext<RoleServer>) -> JobStart<R> {
        JobStart {
            report,
            client_declared_tasks: context
                .client_capabilities()
                .is_some_and(|capabilities| capabilities.supports_tasks()),
        }
    }
}

impl<R: JobReport + Serialize + JsonSchema> IntoCallToolResult for JobStart<R> {
    fn into_call_tool_result(self) -> Result<CallToolResponse, ErrorData> {
        if !self.client_declared_tasks {
            return Json(self.report).into_call_tool_result();
        }
        let detailed = task_of(self.report.job(), || structured(&self.report))?;
        Ok(CallToolResponse::Task(CreateTaskResult::new(detailed.task)))
    }
}

/// The task of a job. `result` gives what `job_status` gives for the job, and is read only when
/// the job has ended. A job that failed is a completed task whose result is a tool error, because
/// a failed task is one that ended in a protocol error and this server never answers a tool
/// failure with one.
fn task_of(
    job: &Job,
    result: impl FnOnce() -> Result<Value, ErrorData>,
) -> Result<DetailedTask, ErrorData> {
    let (payload, message) = match job.state() {
        JobState::Running => (
            TaskPayload::Working,
            job.stage().map(|stage| stage.words.clone()),
        ),
        JobState::Done | JobState::AlreadyIngested => {
            (completed(CallToolResult::structured(result()?))?, None)
        }
        JobState::Failed => (
            completed(CallToolResult::structured_error(result()?))?,
            job.error().map(str::to_owned),
        ),
        JobState::Stopped => (TaskPayload::Cancelled, job.error().map(str::to_owned)),
    };
    let created_at = rfc3339(job.started_at()).map_err(time_error)?;
    let last_updated_at = rfc3339(job.updated_at()).map_err(time_error)?;
    let mut task = Task::new(job.job_id(), payload.status(), created_at, last_updated_at)
        .with_ttl_ms(TASK_TTL_MS)
        .with_poll_interval_ms(POLL_INTERVAL_MS);
    task.status_message = message;
    Ok(DetailedTask::new(task, payload))
}

/// The answer to `tasks/get`.
pub(super) fn get<S: Services>(
    shared: &Shared<S>,
    task_id: &str,
) -> Result<GetTaskResult, ErrorData> {
    let found = FoundJob::find(shared, task_id).map_err(unknown_task)?;
    // A job that has ended never changes again, so the result read after the job agrees with it.
    let job = found.job();
    let detailed = task_of(&job, || structured(&found.result()))?;
    Ok(GetTaskResult::new(detailed))
}

/// The answer to `tasks/cancel`: stops the job as `stop_job` does. A task that has ended already
/// is left as it is.
pub(super) async fn cancel<S: Services>(
    shared: &Shared<S>,
    task_id: &str,
) -> Result<(), ErrorData> {
    stop_any(shared, task_id)
        .await
        .map(|_| ())
        .ok_or_else(|| unknown_task(unknown_job(task_id)))
}

/// The answer to `tasks/update`. A task never asks for input, so there is nothing to give it and
/// the call changes nothing.
pub(super) fn update<S: Services>(shared: &Shared<S>, task_id: &str) -> Result<(), ErrorData> {
    FoundJob::find(shared, task_id)
        .map(|_| ())
        .map_err(unknown_task)
}

fn structured(report: &impl Serialize) -> Result<Value, ErrorData> {
    serde_json::to_value(report).map_err(serialize_error)
}

/// The payload of a task that ended with `result`, written the way a tool call answers it.
fn completed(result: CallToolResult) -> Result<TaskPayload, ErrorData> {
    match serde_json::to_value(result).map_err(serialize_error)? {
        Value::Object(result) => Ok(TaskPayload::Completed { result }),
        other => Err(ErrorData::internal_error(
            format!("a tool result should be a JSON object, but it is {other}"),
            None,
        )),
    }
}

fn unknown_job(job_id: &str) -> StopError {
    StopError::UnknownJob {
        job_id: job_id.to_owned(),
    }
}

/// An id that is no job is a parameter that is wrong, and the text of the error says what to do.
fn unknown_task(error: StopError) -> ErrorData {
    ErrorData::invalid_params(error.to_string(), None)
}

fn time_error(error: time::error::Format) -> ErrorData {
    ErrorData::internal_error(format!("a task time could not be written: {error}"), None)
}

fn serialize_error(error: serde_json::Error) -> ErrorData {
    ErrorData::internal_error(
        format!("the result of a task could not be written: {error}"),
        None,
    )
}
