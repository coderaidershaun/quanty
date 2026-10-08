//! The list of ingest jobs, and the report of each one. A job is one `watch` channel that carries
//! its report: only the tasks of the job write it, and any number of callers read it.

use std::collections::VecDeque;
use std::sync::{Mutex, PoisonError};

use rag_ingestion::PdfOutcome;
use schemars::JsonSchema;
use serde::Serialize;
use tokio::sync::watch;
use uuid::Uuid;

use super::PdfIngestError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum JobState {
    /// The ingest is going on. Ask again in 20 to 30 seconds.
    Running,
    /// The PDF was converted and ingested.
    Done,
    /// Both stores already held this PDF, so nothing was converted or embedded and it cost
    /// nothing. The `document_tags` that were sent are still written.
    AlreadyIngested,
    /// The ingest stopped. `error` says why. Send the same PDF again to go on.
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Stage {
    /// The pages are being read by `claude` and Jev. This is the slow part.
    Converting,
    /// The converted chapter is being embedded, stored and searched for concepts.
    Ingesting,
}

/// How an ingest job is going, or how it ended.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub(crate) struct IngestReport {
    /// Give it to `ingest_status`.
    pub(super) job_id: String,
    pub(super) state: JobState,
    /// Only while `running`. Left out while the stores are being checked, before any paid work.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) stage: Option<Stage>,
    /// The title of the media.
    media: String,
    /// The name of the PDF file.
    file: String,
    /// Give it to `read_page`. When `done` or `already_ingested`.
    #[serde(skip_serializing_if = "Option::is_none")]
    document_id: Option<String>,
    /// How many items the document has in the stores. When `done` or `already_ingested`.
    #[serde(skip_serializing_if = "Option::is_none")]
    items: Option<u64>,
    /// What the ingest did, as `rag-ingest pdf` prints it. When `done`.
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    /// Why the ingest stopped, and what to do. When `failed`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) error: Option<String>,
}

impl IngestReport {
    fn running(job: Uuid, media: &str, file: &str) -> IngestReport {
        IngestReport {
            job_id: job.to_string(),
            state: JobState::Running,
            stage: None,
            media: media.to_owned(),
            file: file.to_owned(),
            document_id: None,
            items: None,
            summary: None,
            error: None,
        }
    }

    /// The end of a job that ran to the end.
    pub(super) fn finish(&mut self, outcome: &PdfOutcome) {
        self.stage = None;
        self.document_id = Some(outcome.doc_id().to_string());
        match outcome {
            PdfOutcome::AlreadyIngested { items, .. } => {
                self.state = JobState::AlreadyIngested;
                self.items = Some(*items);
            }
            PdfOutcome::Ingested(summary) => {
                self.state = JobState::Done;
                self.items = Some(summary.ingest.items_by_kind.total() as u64);
                self.summary = Some(outcome.to_string());
            }
        }
    }

    /// The end of a job that stopped. `error` is the whole text for the agent.
    pub(super) fn fail(&mut self, error: String) {
        self.stage = None;
        self.state = JobState::Failed;
        self.error = Some(error);
    }
}

/// What the tasks of a job get when it starts.
pub(super) struct Started {
    /// The one place a report is written. The tasks of the job hold clones of it.
    pub(super) report: watch::Sender<IngestReport>,
    pub(super) watcher: watch::Receiver<IngestReport>,
}

/// How many jobs the list keeps. Without a limit, a server that runs for months would keep one
/// report for each PDF it was ever sent.
pub(super) const KEPT_JOBS: usize = 100;

/// The newest jobs, oldest first.
#[derive(Default)]
pub(super) struct Jobs {
    list: Mutex<VecDeque<(Uuid, watch::Receiver<IngestReport>)>>,
}

impl Jobs {
    /// Makes a job, unless one is running, and forgets the oldest job when the list is full. The
    /// check and the new job are in one lock with no `.await` between them, so two calls at the
    /// same moment cannot both start. It holds in one server only: a second server, or
    /// `rag-ingest pdf`, is not stopped from taking the same chapter at the same time.
    pub(super) fn start(&self, media: &str, file: &str) -> Result<Started, PdfIngestError> {
        let mut list = self.list.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((running, _)) = list.iter().find(|(_, watcher)| is_running(watcher)) {
            return Err(PdfIngestError::Busy {
                job_id: running.to_string(),
            });
        }
        // No job is running here, so the job that is forgotten has ended.
        if list.len() >= KEPT_JOBS {
            list.pop_front();
        }
        let id = Uuid::new_v4();
        let (report, watcher) = watch::channel(IngestReport::running(id, media, file));
        list.push_back((id, watcher.clone()));
        Ok(Started { report, watcher })
    }

    /// The latest report of the job with this id, or `None` when no job has it.
    pub(super) fn report_of(&self, job_id: &str) -> Option<IngestReport> {
        let wanted = Uuid::parse_str(job_id.trim()).ok()?;
        let list = self.list.lock().unwrap_or_else(PoisonError::into_inner);
        list.iter()
            .find(|(id, _)| *id == wanted)
            .map(|(_, watcher)| watcher.borrow().clone())
    }
}

/// Whether the job is going on. A job whose tasks are gone is not, whatever its report says:
/// otherwise a job that lost its tasks before the end was written would refuse every later PDF.
fn is_running(watcher: &watch::Receiver<IngestReport>) -> bool {
    // `has_changed` fails only when every writer of the report is gone.
    watcher.borrow().state == JobState::Running && watcher.has_changed().is_ok()
}
