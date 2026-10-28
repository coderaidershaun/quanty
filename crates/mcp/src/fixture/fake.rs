//! The fake ingest jobs of the fixture. A job walks the stages of a real ingest on a timer, so
//! that a host can draw a progress bar with no store, model or PDF behind it.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use contract::IngestStage;
use schemars::JsonSchema;
use serde::Serialize;
use tokio::time::{Instant, sleep_until};
use uuid::Uuid;

use crate::ingest::stage_view;
use crate::jobs::{Job, JobKind, JobState, ProgressSender, ProgressSink, lock};

/// How long a fake job stays in each stage.
const STAGE_STEP: Duration = Duration::from_millis(150);

/// The stages of a real ingest of a seven-page chapter, in order, each shown at the end of its
/// work.
const STAGES: [IngestStage; 11] = [
    IngestStage::CheckingStored,
    IngestStage::OpeningPdf,
    IngestStage::PreparingPages {
        ready: 7,
        saved_before: 0,
        pages: 7,
    },
    IngestStage::Converting { saved: 7, pages: 7 },
    IngestStage::WritingArtifacts {
        done: 20,
        items: 20,
    },
    IngestStage::WritingGraph,
    IngestStage::Embedding { items: 20 },
    IngestStage::Storing,
    IngestStage::WritingInsights,
    IngestStage::ReadingConcepts {
        done: 20,
        items: 20,
    },
    IngestStage::LinkingConcepts {
        done: 20,
        items: 20,
    },
];

/// Every fake job that was started, shared by all the sessions of one fixture.
// SMELL: a job is never removed, so the list grows with every fake ingest for as long as the
// fixture runs. That is fine for acceptance tests, but not for a fixture left running for days.
#[derive(Clone, Default)]
pub(super) struct FakeJobs(Arc<Mutex<Vec<FakeJob>>>);

impl FakeJobs {
    /// Starts a job that is in its first stage now.
    pub(super) fn start(&self, media: String, file_name: String) -> FakeJob {
        let job = FakeJob {
            job: Job::running(Uuid::new_v4(), JobKind::Ingest),
            media,
            file_name,
            started: Instant::now(),
        };
        lock(&self.0).push(job.clone());
        job
    }

    /// The job with this id, or `None` when no job has it.
    pub(super) fn find(&self, job_id: &str) -> Option<FakeJob> {
        lock(&self.0)
            .iter()
            .find(|job| job.job.job_id() == job_id.trim())
            .cloned()
    }

    /// Every job, oldest first.
    pub(super) fn all(&self) -> Vec<FakeJob> {
        lock(&self.0).clone()
    }
}

/// Where a fake job is on its timer.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Stage(usize),
    Done,
}

#[derive(Clone)]
pub(super) struct FakeJob {
    job: Job,
    media: String,
    file_name: String,
    started: Instant,
}

/// What a fake job looks like to a tool call: the `Job` of a real ingest, with the media and the
/// name of the file that was sent.
#[derive(Serialize, JsonSchema)]
pub(super) struct FakeIngest {
    #[serde(flatten)]
    job: Job,
    media: String,
    file_name: String,
}

impl FakeJob {
    pub(super) fn job_id(&self) -> &str {
        self.job.job_id()
    }

    pub(super) fn file_name(&self) -> &str {
        &self.file_name
    }

    fn phase(&self) -> Phase {
        let steps = self.started.elapsed().as_millis() / STAGE_STEP.as_millis();
        match usize::try_from(steps) {
            Ok(index) if index < STAGES.len() => Phase::Stage(index),
            _ => Phase::Done,
        }
    }

    /// The job as it is now.
    pub(super) fn view(&self) -> FakeIngest {
        let mut job = self.job.clone();
        match self.phase() {
            Phase::Stage(index) => job.set_stage(stage_view(STAGES[index])),
            Phase::Done => job.end(JobState::Done),
        }
        FakeIngest {
            job,
            media: self.media.clone(),
            file_name: self.file_name.clone(),
        }
    }

    /// The job once it is done or once `timeout` has passed. Each stage from the one the job is in
    /// when the wait begins is sent to `progress`, when there is one, once and in order.
    pub(super) async fn watch(
        &self,
        timeout: Duration,
        progress: Option<ProgressSink>,
    ) -> FakeIngest {
        let deadline = Instant::now() + timeout;
        let mut unsent = match self.phase() {
            Phase::Stage(index) => index,
            Phase::Done => STAGES.len(),
        };
        let mut progress = progress.map(ProgressSender::new);
        let mut job = self.job.clone();
        loop {
            let phase = self.phase();
            let reached = match phase {
                Phase::Stage(index) => index + 1,
                Phase::Done => STAGES.len(),
            };
            // A wait that wakes late still sends every stage it slept through.
            for &stage in &STAGES[unsent..reached] {
                job.set_stage(stage_view(stage));
                if let Some(sender) = progress.as_mut()
                    && !sender.send_if_changed(&job).await
                {
                    progress = None;
                }
            }
            unsent = reached;
            let Phase::Stage(index) = phase else {
                return self.view();
            };
            let next_stage = self.started + STAGE_STEP * (index as u32 + 1);
            if Instant::now() >= deadline {
                return self.view();
            }
            sleep_until(next_stage.min(deadline)).await;
        }
    }
}
