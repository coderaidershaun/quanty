//! The report of one dataset build, as `build_dataset` and `dataset_status` give it.

use std::path::Path;

use rag_core::UsageTally;
use rag_dataset::{GenerationKind, Step};
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use crate::jobs::{Job, JobKind, JobReport, JobState, Stage};

/// How a dataset build is going, or how it ended. When `done`, the build wrote every file of the
/// dataset and `summary` says what it holds. What the models used until a `stopped` build is in
/// `usage`.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub(crate) struct DatasetReport {
    #[serde(flatten)]
    job: Job,
    /// The folder the dataset's files are written to.
    folder: String,
    /// What the build wrote, as `rag-dataset build` prints it. When `done`.
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
}

impl DatasetReport {
    pub(super) fn running(job: Uuid, folder: &Path) -> DatasetReport {
        let mut job = Job::running(job, JobKind::Dataset);
        job.set_spent(Some((&UsageTally::default()).into()));
        DatasetReport {
            job,
            folder: folder.display().to_string(),
            summary: None,
        }
    }

    /// One step of the build, with what the models used so far.
    pub(super) fn progress(&mut self, step: Step, usage: &UsageTally) {
        let counted = counts(step);
        let stage = Stage::new(stage_name(step), stage_words(step)).with_counts(
            counted.map(|(done, _)| done as u64),
            counted.map(|(_, total)| total as u64),
        );
        self.job.set_stage(stage);
        self.job.set_spent(Some(usage.into()));
    }

    /// The end of a build that ran to the end.
    pub(super) fn finish(&mut self, built: &rag_dataset::Report) {
        self.summary = Some(built.to_string());
        self.job.set_spent(Some((&built.usage).into()));
        self.job.end(JobState::Done);
    }

    /// The end of a build that stopped. `error` is the whole text for the agent. What was spent
    /// until then stays.
    pub(super) fn fail(&mut self, error: String) {
        self.job.fail(error);
    }
}

impl JobReport for DatasetReport {
    fn job(&self) -> &Job {
        &self.job
    }

    fn stopped(&mut self) {
        self.job.stop(
            "stopped by `stop_job`; call `build_dataset` again: the answers a model already gave are kept and not paid for twice."
                .to_owned(),
        );
    }
}

fn stage_name(step: Step) -> &'static str {
    match step {
        Step::Exporting { .. } => "exporting",
        Step::Checking => "checking",
        Step::Planning => "planning",
        Step::Writing {
            kind: GenerationKind::Synthetic,
            ..
        } => "writing_synthetic",
        Step::Writing {
            kind: GenerationKind::Sft,
            ..
        } => "writing_sft",
        Step::WritingManifest => "writing_manifest",
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
