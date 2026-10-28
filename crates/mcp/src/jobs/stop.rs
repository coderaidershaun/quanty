//! What the `stop_job` tool takes and gives back.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::KEPT_JOBS;
use super::background::JobKind;

/// The tool that follows a job of any kind.
const STATUS_TOOL: &str = "job_status";

/// What an agent gives `stop_job`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct StopJobArgs {
    /// The `job_id` of a job.
    pub(crate) job_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StopState {
    /// The job was running and is stopped now.
    Stopped,
    /// The job had ended already, and was left as it was.
    Ended,
}

/// What `stop_job` did.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct StopResult {
    job_id: String,
    kind: JobKind,
    state: StopState,
    /// The tool that says how the job ended and what to do next.
    status_tool: &'static str,
}

impl StopResult {
    pub(crate) fn new(job_id: &str, kind: JobKind, state: StopState) -> StopResult {
        StopResult {
            job_id: job_id.trim().to_owned(),
            kind,
            state,
            status_tool: STATUS_TOOL,
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum StopError {
    #[error(
        "no job of this server has the id {job_id:?}; job ids are forgotten when the server restarts, and the server keeps only its last {KEPT_JOBS} jobs of each kind"
    )]
    UnknownJob { job_id: String },
}
