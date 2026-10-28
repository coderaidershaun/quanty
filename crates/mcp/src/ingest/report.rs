//! The report of one ingest job, as `ingest_pdf` and `ingest_status` give it.

use contract::IngestStage;
use rag_ingestion::PdfOutcome;
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use super::GO_ON;
use super::progress::stage_view;
use crate::jobs::{Job, JobKind, JobReport, JobState};
use crate::usage::Spent;

/// How an ingest job is going, or how it ended.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub(crate) struct IngestReport {
    #[serde(flatten)]
    job: Job,
    /// The latest stage of the ingest. `ingest_pdf` reads it to know when the paid work has begun,
    /// and `job` holds the same stage as the words and counts that an agent reads.
    #[serde(skip)]
    stage: Option<IngestStage>,
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
}

impl IngestReport {
    pub(super) fn running(job: Uuid, media: &str, file: &str) -> IngestReport {
        IngestReport {
            job: Job::running(job, JobKind::Ingest),
            stage: None,
            media: media.to_owned(),
            artifacts_done: None,
            artifacts_total: None,
            file: file.to_owned(),
            document_id: None,
            items: None,
            summary: None,
        }
    }

    /// The end of a job that ran to the end.
    pub(super) fn finish(&mut self, outcome: &PdfOutcome) {
        self.stage = None;
        self.document_id = Some(outcome.doc_id().to_string());
        let (state, spent) = match outcome {
            PdfOutcome::AlreadyIngested {
                items,
                insights,
                artifacts,
                ..
            } => {
                self.items = Some(*items);
                self.artifacts_done = Some(artifacts.total);
                self.artifacts_total = Some(artifacts.total);
                let tried = insights
                    .as_ref()
                    .is_some_and(|insights| insights.outcome.tried_to_write());
                let spent = if tried || artifacts.written > 0 {
                    self.summary = Some(outcome.to_string());
                    Some(Spent::from(&outcome.usage()))
                } else {
                    None
                };
                (JobState::AlreadyIngested, spent)
            }
            PdfOutcome::Ingested(summary) => {
                self.items = Some(summary.ingest.items_by_kind.total() as u64);
                self.summary = Some(outcome.to_string());
                (JobState::Done, Some(Spent::from(&summary.usage())))
            }
        };
        self.job.set_spent(spent);
        self.job.end(state);
    }

    /// One step of the ingest: what the run has used, the counts of the artifact descriptions when
    /// the step is theirs, and the stage the step turns into, when it does.
    pub(super) fn progress(
        &mut self,
        spent: &rag_core::UsageTally,
        artifacts: Option<(usize, usize)>,
        stage: Option<IngestStage>,
    ) {
        self.job.set_spent(Some(spent.into()));
        if let Some((done, total)) = artifacts {
            self.artifacts_done = Some(done);
            self.artifacts_total = Some(total);
        }
        if let Some(stage) = stage {
            self.stage = Some(stage);
            self.job.set_stage(stage_view(stage));
        }
    }

    /// The end of a job that stopped. `error` is the whole text for the agent.
    pub(super) fn fail(&mut self, error: String) {
        self.stage = None;
        self.job.fail(error);
    }

    /// Whether `ingest_pdf` can answer with this report. Nothing is paid for while the stores are
    /// checked, so it waits for a later stage. A PDF that is stored already can end with no later
    /// stage, so the end of the job answers too.
    pub(super) fn is_ready_to_answer(&self) -> bool {
        self.stage
            .is_some_and(|stage| stage != IngestStage::CheckingStored)
            || !self.is_running()
    }
}

impl JobReport for IngestReport {
    fn job(&self) -> &Job {
        &self.job
    }

    fn stopped(&mut self) {
        self.stage = None;
        self.job.stop(format!("stopped by `stop_job`. {GO_ON}"));
    }
}
