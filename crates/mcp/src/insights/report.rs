//! The report of one insights job, as `write_insights` and `insights_status` give it.

use std::path::Path;

use rag_core::UsageTally;
use rag_ingestion::StoredChapter;
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use crate::jobs::JobReport;
use crate::usage::Spent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InsightsState {
    /// The job is going on. Ask again in 20 to 30 seconds.
    Running,
    /// Every chosen document was handled. A document that is `not_written` does not fail the job:
    /// its `reason` says why, and calling `write_insights` again tries it once more.
    Done,
    /// The job stopped. `error` says why and what to do.
    Failed,
    /// `stop_job` stopped the job. `error` says what to do. What was written until then is kept.
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Outcome {
    /// A dry run: the insights would be written, and nothing was asked.
    WouldWrite,
    /// The insights were written to `file`.
    Written,
    /// The insights were saved already, so nothing was asked.
    AlreadySaved,
    /// Nothing was saved, and `reason` says why. Calling `write_insights` again tries once more.
    NotWritten,
    /// A part of a book that is not a chapter, such as its index, gets none.
    NoneForBookPart,
    /// The graph holds the document, but the content folder holds no converted chapter of it.
    NoConvertedChapter,
}

/// What became of one document.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub(crate) struct DocumentLine {
    /// Give it to `chapter_insights`.
    document_id: String,
    /// The whole stored title of the document.
    title: String,
    outcome: Outcome,
    /// The file the insights were written to, or that a dry run would write.
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    /// Why nothing was saved or found.
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

impl DocumentLine {
    pub(super) fn new(chapter: &StoredChapter, outcome: Outcome) -> DocumentLine {
        DocumentLine {
            document_id: chapter.doc_id.to_string(),
            title: chapter.title.clone(),
            outcome,
            file: None,
            reason: None,
        }
    }

    pub(super) fn with_file(self, file: &Path) -> DocumentLine {
        DocumentLine {
            file: Some(file.display().to_string()),
            ..self
        }
    }

    pub(super) fn with_reason(self, reason: String) -> DocumentLine {
        DocumentLine {
            reason: Some(reason),
            ..self
        }
    }
}

/// How an insights job is going, or how it ended.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub(crate) struct InsightsReport {
    /// Give it to `insights_status` and `stop_job`.
    job_id: String,
    pub(super) state: InsightsState,
    /// True when the job only lists what it would write.
    dry_run: bool,
    /// How many of the chosen documents are handled.
    done: usize,
    /// How many documents were chosen.
    total: usize,
    /// The title of the document the agent is writing the insights of. Only while a question is
    /// out.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) current: Option<String>,
    /// What became of each document handled so far, in the order they were handled.
    documents: Vec<DocumentLine>,
    /// What the questions used so far; what the whole job used when it has ended.
    #[serde(flatten)]
    spent: Spent,
    /// Why the job stopped, and what to do. When `failed` or `stopped`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) error: Option<String>,
}

impl InsightsReport {
    pub(super) fn running(job: Uuid, dry_run: bool, total: usize) -> InsightsReport {
        InsightsReport {
            job_id: job.to_string(),
            state: InsightsState::Running,
            dry_run,
            done: 0,
            total,
            current: None,
            documents: Vec::new(),
            spent: (&UsageTally::default()).into(),
            error: None,
        }
    }

    /// A question about this document is going out.
    pub(super) fn asking(&mut self, title: &str) {
        self.current = Some(title.to_owned());
    }

    /// One document is handled. `usage` is what the job has used so far.
    pub(super) fn handled(&mut self, line: DocumentLine, usage: &UsageTally) {
        self.documents.push(line);
        self.done += 1;
        self.current = None;
        self.spent = usage.into();
    }

    /// The end of a job that went through every chosen document.
    pub(super) fn finish(&mut self) {
        self.state = InsightsState::Done;
        self.current = None;
    }

    /// The end of a job that stopped. `error` is the whole text for the agent.
    pub(super) fn fail(&mut self, error: String) {
        self.state = InsightsState::Failed;
        self.current = None;
        self.error = Some(error);
    }
}

impl JobReport for InsightsReport {
    fn is_running(&self) -> bool {
        self.state == InsightsState::Running
    }

    fn stopped(&mut self) {
        self.state = InsightsState::Stopped;
        self.current = None;
        self.error = Some(
            "stopped by `stop_job`; call `write_insights` again: insights that are saved are not asked for twice."
                .to_owned(),
        );
    }

    fn is_stopped(&self) -> bool {
        self.state == InsightsState::Stopped
    }
}
