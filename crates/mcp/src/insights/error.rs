//! Everything that stops an insights tool: an argument that is refused, a saved file that cannot
//! be read, a job that cannot start, and a job that fails while it runs.

use graph::GraphError;
use ocr::ContentError;
use rag_core::LlmError;
use tokio::task::JoinError;

use crate::jobs::{BackgroundJob, Busy, KEPT_JOBS};
use crate::library::LibraryError;

#[derive(thiserror::Error, Debug)]
pub(crate) enum InsightsToolError {
    #[error(transparent)]
    Library(#[from] LibraryError),

    #[error(
        "could not read the saved insights of the document; call `write_insights` with this `document_id` to write them again, which is paid"
    )]
    Unreadable(#[source] ContentError),

    #[error(
        "could not read from the graph whether this document is a part of a book, which gets no insights"
    )]
    Category(#[source] GraphError),

    #[error("give exactly one of `document_id`, `media` and `all: true`")]
    Choice,

    #[error("could not connect to the graph")]
    Graph(#[from] GraphError),

    #[error(
        "the agent of the Ingest choice cannot be asked, so no insights were written; sign it in, or give `dry_run: true`, which asks no model"
    )]
    ModelNotReady(#[source] LlmError),

    #[error(
        "insights are being written (job {job_id}); ask `insights_status` with that job_id, and call `write_insights` again when that job has ended"
    )]
    Busy { job_id: String },

    #[error(
        "background work is running: {kind} of {name}; wait for it to end before writing insights"
    )]
    SharedBusy { kind: &'static str, name: String },

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

impl From<Busy> for InsightsToolError {
    fn from(busy: Busy) -> InsightsToolError {
        InsightsToolError::Busy {
            job_id: busy.running_job.to_string(),
        }
    }
}

impl From<BackgroundJob> for InsightsToolError {
    fn from(job: BackgroundJob) -> Self {
        Self::SharedBusy {
            kind: job.kind.label(),
            name: job.name,
        }
    }
}
