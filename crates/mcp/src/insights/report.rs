//! The report of one insights job, as `write_insights` and `insights_status` give it.

use std::path::Path;

use rag_core::UsageTally;
use rag_ingestion::StoredChapter;
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use crate::jobs::{Job, JobKind, JobReport, JobState, Stage};

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

/// How an insights job is going, or how it ended. When `done`, every chosen document was handled:
/// a document that is `not_written` does not fail the job, its `reason` says why, and calling
/// `write_insights` again tries it once more. What a `stopped` job wrote is kept.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub(crate) struct InsightsReport {
    #[serde(flatten)]
    job: Job,
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
}

impl InsightsReport {
    pub(super) fn running(job: Uuid, dry_run: bool, total: usize) -> InsightsReport {
        let mut job = Job::running(job, JobKind::Insights);
        job.set_spent(Some((&UsageTally::default()).into()));
        InsightsReport {
            job,
            dry_run,
            done: 0,
            total,
            current: None,
            documents: Vec::new(),
        }
    }

    /// A question about this document is going out.
    pub(super) fn asking(&mut self, title: &str) {
        self.current = Some(title.to_owned());
        self.show_stage();
    }

    /// One document is handled. `usage` is what the job has used so far.
    pub(super) fn handled(&mut self, line: DocumentLine, usage: &UsageTally) {
        self.documents.push(line);
        self.done += 1;
        self.current = None;
        self.job.set_spent(Some(usage.into()));
        self.show_stage();
    }

    /// The end of a job that went through every chosen document.
    pub(super) fn finish(&mut self) {
        self.current = None;
        self.job.end(JobState::Done);
    }

    /// The end of a job that stopped. `error` is the whole text for the agent.
    pub(super) fn fail(&mut self, error: String) {
        self.current = None;
        self.job.fail(error);
    }

    fn show_stage(&mut self) {
        let counts = format!(
            "writing insights: {} of {} documents done",
            self.done, self.total
        );
        let words = match &self.current {
            Some(title) => format!("{counts}, asking about {title}"),
            None => counts,
        };
        let stage = Stage::new("writing_insights", words)
            .with_counts(Some(self.done as u64), Some(self.total as u64));
        self.job.set_stage(stage);
    }
}

impl JobReport for InsightsReport {
    fn job(&self) -> &Job {
        &self.job
    }

    fn stopped(&mut self) {
        self.current = None;
        self.job.stop(
            "stopped by `stop_job`; call `write_insights` again: insights that are saved are not asked for twice."
                .to_owned(),
        );
    }
}
