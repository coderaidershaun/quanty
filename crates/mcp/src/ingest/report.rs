//! The report of one ingest job, as `ingest_pdf` and `ingest_status` give it.

use rag_ingestion::PdfOutcome;
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use super::GO_ON;
use crate::jobs::JobReport;
use crate::usage::Spent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum JobState {
    /// The ingest is going on. Ask again in 20 to 30 seconds.
    Running,
    /// The PDF was converted and ingested.
    Done,
    /// Both stores already held this PDF, so nothing was converted, embedded or stored. When the
    /// artifact descriptions or enabled insights need repair, `summary` and `usage` report that work.
    /// Supplied `document_tags` are still written.
    AlreadyIngested,
    /// The ingest stopped. `error` says why. Send the same PDF again to go on.
    Failed,
    /// `stop_job` stopped the ingest. `error` says what to do. Send the same PDF again to go on.
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Stage {
    /// The pages are being read by `claude` and Jev. This is the slow part.
    Converting,
    /// The converted chapter is being embedded, stored and searched for concepts.
    Ingesting,
    /// The agent of the Ingest choice is writing the chapter's insights, takeaways and cheat
    /// sheet. A PDF that was ingested before insights were written is in this stage while its
    /// missing insights are written.
    WritingInsights,
    /// Required artifact titles and descriptions are being written.
    WritingArtifacts,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    artifacts_done: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    artifacts_total: Option<usize>,
    /// The name of the PDF file.
    file: String,
    /// Give it to `read_page`. When `done` or `already_ingested`.
    #[serde(skip_serializing_if = "Option::is_none")]
    document_id: Option<String>,
    /// How many items the document has in the stores. When `done` or `already_ingested`.
    #[serde(skip_serializing_if = "Option::is_none")]
    items: Option<u64>,
    /// What the ingest did, as `rag-ingest pdf` prints it. When `done`, and when
    /// `already_ingested` wrote artifact descriptions or tried to write insights.
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    /// Cumulative usage, retained on failure. Completed repeat runs include it when they wrote
    /// artifact descriptions or tried to write insights.
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    spent: Option<Spent>,
    /// Why the ingest stopped, and what to do. When `failed` or `stopped`.
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
            artifacts_done: None,
            artifacts_total: None,
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
            PdfOutcome::AlreadyIngested {
                items,
                insights,
                artifacts,
                ..
            } => {
                self.state = JobState::AlreadyIngested;
                self.items = Some(*items);
                self.artifacts_done = Some(artifacts.total);
                self.artifacts_total = Some(artifacts.total);
                let tried = insights
                    .as_ref()
                    .is_some_and(|insights| insights.outcome.tried_to_write());
                if tried || artifacts.written > 0 {
                    self.summary = Some(outcome.to_string());
                    self.spent = Some((&outcome.usage()).into());
                } else {
                    self.spent = None;
                }
            }
            PdfOutcome::Ingested(summary) => {
                self.state = JobState::Done;
                self.items = Some(summary.ingest.items_by_kind.total() as u64);
                self.summary = Some(outcome.to_string());
                self.spent = Some((&summary.usage()).into());
            }
        }
    }

    pub(super) fn progress(
        &mut self,
        spent: &rag_core::UsageTally,
        artifacts: Option<(usize, usize)>,
    ) {
        self.spent = Some(spent.into());
        if let Some((done, total)) = artifacts {
            self.artifacts_done = Some(done);
            self.artifacts_total = Some(total);
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

    fn stopped(&mut self) {
        self.stage = None;
        self.state = JobState::Stopped;
        self.error = Some(format!("stopped by `stop_job`. {GO_ON}"));
    }

    fn is_stopped(&self) -> bool {
        self.state == JobState::Stopped
    }
}
