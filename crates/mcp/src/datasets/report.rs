//! The report of one dataset build, as `build_dataset` and `dataset_status` give it.

use std::path::Path;

use rag_core::UsageTally;
use rag_dataset::{GenerationKind, Step};
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use crate::jobs::JobReport;
use crate::usage::Spent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DatasetState {
    /// The build is going on. Ask again in 20 to 30 seconds.
    Running,
    /// The build wrote every file of the dataset. `summary` says what it holds.
    Done,
    /// The build stopped. `error` says why and what to do.
    Failed,
}

/// How a dataset build is going, or how it ended.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub(crate) struct DatasetReport {
    /// Give it to `dataset_status`.
    job_id: String,
    pub(super) state: DatasetState,
    /// What the build is doing, in words, such as "exporting 3 of 12 documents". Only while
    /// `running`, and left out before the first step.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) stage: Option<String>,
    /// How many of `total` the stage has done, for a stage that counts. Only while `running`.
    #[serde(skip_serializing_if = "Option::is_none")]
    done: Option<usize>,
    /// How many the stage counts to, for a stage that counts. Only while `running`.
    #[serde(skip_serializing_if = "Option::is_none")]
    total: Option<usize>,
    /// The folder the dataset's files are written to.
    folder: String,
    /// What the models used so far, updated while the build runs; what the whole build used when
    /// `done`.
    #[serde(flatten)]
    spent: Spent,
    /// What the build wrote, as `rag-dataset build` prints it. When `done`.
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    /// Why the build stopped, and what to do. When `failed`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) error: Option<String>,
}

impl DatasetReport {
    pub(super) fn running(job: Uuid, folder: &Path) -> DatasetReport {
        DatasetReport {
            job_id: job.to_string(),
            state: DatasetState::Running,
            stage: None,
            done: None,
            total: None,
            folder: folder.display().to_string(),
            spent: (&UsageTally::default()).into(),
            summary: None,
            error: None,
        }
    }

    /// One step of the build, with what the models used so far.
    pub(super) fn progress(&mut self, step: Step, usage: &UsageTally) {
        let counted = counts(step);
        self.stage = Some(stage_words(step));
        self.done = counted.map(|(done, _)| done);
        self.total = counted.map(|(_, total)| total);
        self.spent = usage.into();
    }

    /// The end of a build that ran to the end.
    pub(super) fn finish(&mut self, built: &rag_dataset::Report) {
        self.end(DatasetState::Done);
        self.summary = Some(built.to_string());
        self.spent = (&built.usage).into();
    }

    /// The end of a build that stopped. `error` is the whole text for the agent. What was spent
    /// until then stays.
    pub(super) fn fail(&mut self, error: String) {
        self.end(DatasetState::Failed);
        self.error = Some(error);
    }

    fn end(&mut self, state: DatasetState) {
        self.state = state;
        self.stage = None;
        self.done = None;
        self.total = None;
    }
}

impl JobReport for DatasetReport {
    fn is_running(&self) -> bool {
        self.state == DatasetState::Running
    }
}

fn stage_words(step: Step) -> String {
    match step {
        Step::Exporting { done, total } => format!("exporting {done} of {total} documents"),
        Step::Checking => "checking the sections".to_owned(),
        Step::Planning => "planning the generation".to_owned(),
        Step::Writing {
            kind: GenerationKind::Synthetic,
            done,
            total,
        } => format!("writing {done} of {total} synthetic texts"),
        Step::Writing {
            kind: GenerationKind::Sft,
            done,
            total,
        } => format!("writing {done} of {total} SFT examples"),
        Step::WritingManifest => "writing the manifest".to_owned(),
    }
}

/// How many the step has done and how many it counts to, for a step that counts.
fn counts(step: Step) -> Option<(usize, usize)> {
    match step {
        Step::Exporting { done, total } | Step::Writing { done, total, .. } => Some((done, total)),
        Step::Checking | Step::Planning | Step::WritingManifest => None,
    }
}
