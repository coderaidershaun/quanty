//! The report of one ingest job, as `ingest_pdf` and `ingest_status` give it.

use rag_ingestion::PdfOutcome;
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use crate::jobs::JobReport;
use crate::usage::Spent;

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
    /// What the ingest used. When `done`.
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    spent: Option<Spent>,
    /// Why the ingest stopped, and what to do. When `failed`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) error: Option<String>,
}

impl IngestReport {
    pub(super) fn running(job: Uuid, media: &str, file: &str) -> IngestReport {
        IngestReport {
            job_id: job.to_string(),
            state: JobState::Running,
            stage: None,
            media: media.to_owned(),
            file: file.to_owned(),
            document_id: None,
            items: None,
            summary: None,
            spent: None,
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
                self.spent = Some((&summary.usage()).into());
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

impl JobReport for IngestReport {
    fn is_running(&self) -> bool {
        self.state == JobState::Running
    }
}
