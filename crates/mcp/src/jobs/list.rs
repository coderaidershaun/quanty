//! What `list_jobs` gives: the job that holds the shared background slot, and the recent jobs of
//! every kind.

use schemars::JsonSchema;
use serde::Serialize;

use super::{BackgroundJob, Job, JobKind};

/// The background work that holds the one slot of the HTTP server, which keeps every other
/// ingest, insights job and dataset build from starting.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct RunningWork {
    kind: JobKind,
    /// What the work is for, such as the name of the PDF file or of the dataset.
    name: String,
}

/// What `list_jobs` gives.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct JobList {
    /// The work that holds the shared background slot, started here or by the browser. `null` when
    /// the slot is free, and always `null` over stdio, which has no shared slot.
    running: Option<RunningWork>,
    /// The latest jobs of this server of every kind, newest first.
    jobs: Vec<Job>,
}

impl JobList {
    /// Sorts `jobs` newest first, whatever order they come in.
    pub(crate) fn new(running: Option<BackgroundJob>, mut jobs: Vec<Job>) -> JobList {
        jobs.sort_by_key(|job| std::cmp::Reverse(job.started_at()));
        JobList {
            running: running.map(|work| RunningWork {
                kind: work.kind,
                name: work.name,
            }),
            jobs,
        }
    }
}
