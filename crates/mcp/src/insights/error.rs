//! Everything that stops an insights tool: an argument that is refused, a job that cannot start,
//! and a job that fails while it runs.

use graph::GraphError;
use rag_core::LlmError;
use tokio::task::JoinError;

use crate::jobs::BackgroundJob;
use crate::jobs::{Busy, KEPT_JOBS, busy_from};
use crate::library::LibraryError;

#[derive(thiserror::Error, Debug)]
pub(crate) enum InsightsToolError {
    #[error(transparent)]
    Library(#[from] LibraryError),

    #[error("give exactly one of `document_id`, `media` and `all: true`")]
    Choice,

    #[error("could not connect to the graph")]
    Graph(#[from] GraphError),

    #[error(
        "the agent of the Ingest choice cannot be asked, so no insights were written; sign it in, or give `dry_run: true`, which asks no model"
    )]
    ModelNotReady(#[source] LlmError),

    #[error(
        "insights are being written (job {}); ask `job_status` with that job_id, and call `write_insights` again when that job has ended",
        .0.running_job
    )]
    Busy(Busy),

    #[error("background work is running: {0}; wait for it to end before writing insights")]
    SharedBusy(BackgroundJob),

    #[error(
        "no insights job has the id {job_id:?}; job ids are forgotten when the server restarts, and the server keeps only its last {KEPT_JOBS} jobs, so call `write_insights` again: insights that are saved are not asked for twice"
    )]
    UnknownJob { job_id: String },

    #[error(
        "the insights job stopped before it reported; call `write_insights` again: insights that are saved are not asked for twice"
    )]
    Lost,

    /// The finished text of a job that failed, which already says what to do.
    #[error("{0}")]
    Failed(String),

    #[error(
        "the insights job stopped without an answer; call `write_insights` again: insights that are saved are not asked for twice"
    )]
    Stopped(#[from] JoinError),
}

impl InsightsToolError {
    /// Whether the error says that the document asked for is not there, as opposed to an argument
    /// that is refused or a file or store that fails.
    pub(crate) fn is_not_found(&self) -> bool {
        matches!(self, InsightsToolError::Library(error) if error.is_not_found())
    }
}

busy_from!(InsightsToolError);
