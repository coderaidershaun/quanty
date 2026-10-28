//! The one shape that every job tool gives for a job of any kind, so that an agent or a host
//! reads a progress bar the same way for an ingest, an insights job and a dataset build.

use schemars::JsonSchema;
use serde::{Serialize, Serializer};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use super::background::JobKind;
use crate::usage::Spent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum JobState {
    /// The job is going on. Call `watch_job` to wait for the next change, or `job_status` to look
    /// again at once.
    Running,
    /// The job ran to the end. The fields of its kind say what it made.
    Done,
    /// Only for an ingest. Both stores already held this PDF, so nothing was converted, embedded
    /// or stored. When the artifact descriptions or enabled insights needed repair, `summary` and
    /// `usage` report that work.
    AlreadyIngested,
    /// The job stopped. `error` says why and what to do.
    Failed,
    /// `stop_job` stopped the job. `error` says what to do.
    Stopped,
}

impl JobState {
    /// The state as a tool result writes it.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            JobState::Running => "running",
            JobState::Done => "done",
            JobState::AlreadyIngested => "already_ingested",
            JobState::Failed => "failed",
            JobState::Stopped => "stopped",
        }
    }
}

/// What a running job is doing now, with the counts of the stage when it counts.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub(crate) struct Stage {
    /// The stage in a word that does not change between versions, such as `converting`. An ingest
    /// uses the names of the browser's progress bar.
    pub(crate) name: &'static str,
    /// The stage in a few lower-case words for a person, such as `converting the pages: 3 of 7
    /// done`.
    pub(crate) words: String,
    /// How many of `total` the stage has done. `null` for a stage that does not count.
    done: Option<u64>,
    /// How many the stage counts to. `null` for a stage that does not count.
    total: Option<u64>,
    /// How much of the stage is done, from 0 to 1, to three decimals. `null` for a stage that does
    /// not count.
    pub(crate) share: Option<f64>,
}

impl Stage {
    /// A stage that does not count.
    pub(crate) fn new(name: &'static str, words: String) -> Stage {
        Stage {
            name,
            words,
            done: None,
            total: None,
            share: None,
        }
    }

    /// The same stage with its counts. The share is `done` of `total`, and none when the stage has
    /// no `done` or `total` is 0.
    pub(crate) fn with_counts(self, done: Option<u64>, total: Option<u64>) -> Stage {
        let share = match (done, total) {
            (Some(done), Some(total)) if total > 0 => Some(done as f32 / total as f32),
            _ => None,
        };
        Stage {
            done,
            total,
            ..self
        }
        .with_share(share)
    }

    /// The same stage with another share, for a stage whose bar is not `done` of `total`.
    pub(crate) fn with_share(self, share: Option<f32>) -> Stage {
        Stage {
            share: share.map(|share| (f64::from(share) * 1000.0).round() / 1000.0),
            ..self
        }
    }
}

/// How a job is going, or how it ended: the same fields for every kind of job.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub(crate) struct Job {
    /// Give it to `job_status`, `watch_job` and `stop_job`.
    job_id: String,
    kind: JobKind,
    state: JobState,
    /// Only while `running`. Left out before the first step.
    #[serde(skip_serializing_if = "Option::is_none")]
    stage: Option<Stage>,
    /// When the job was made, as RFC 3339 text in UTC.
    #[serde(serialize_with = "as_rfc3339")]
    #[schemars(with = "String")]
    started_at: OffsetDateTime,
    /// When the report of the job last changed, as RFC 3339 text in UTC.
    #[serde(serialize_with = "as_rfc3339")]
    #[schemars(with = "String")]
    updated_at: OffsetDateTime,
    /// What the models used so far, and what the whole job used when it has ended. Left out when
    /// the job has used nothing to report, such as an ingest that found its PDF stored.
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    spent: Option<Spent>,
    /// Why the job stopped, and what to do. When `failed` or `stopped`.
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl Job {
    pub(crate) fn running(id: Uuid, kind: JobKind) -> Job {
        let now = OffsetDateTime::now_utc();
        Job {
            job_id: id.to_string(),
            kind,
            state: JobState::Running,
            stage: None,
            started_at: now,
            updated_at: now,
            spent: None,
            error: None,
        }
    }

    pub(crate) fn state(&self) -> JobState {
        self.state
    }

    pub(crate) fn stage(&self) -> Option<&Stage> {
        self.stage.as_ref()
    }

    pub(crate) fn started_at(&self) -> OffsetDateTime {
        self.started_at
    }

    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub(crate) fn set_stage(&mut self, stage: Stage) {
        self.stage = Some(stage);
        self.touch();
    }

    pub(crate) fn set_spent(&mut self, spent: Option<Spent>) {
        self.spent = spent;
        self.touch();
    }

    /// The end of the job in `state`. A job that has ended has no stage.
    pub(crate) fn end(&mut self, state: JobState) {
        self.state = state;
        self.stage = None;
        self.touch();
    }

    /// The end of a job that failed. `error` is the whole text for the agent.
    pub(crate) fn fail(&mut self, error: String) {
        self.end(JobState::Failed);
        self.error = Some(error);
    }

    /// The end of a job that `stop_job` stopped. `error` says what to do.
    pub(crate) fn stop(&mut self, error: String) {
        self.end(JobState::Stopped);
        self.error = Some(error);
    }

    fn touch(&mut self) {
        self.updated_at = OffsetDateTime::now_utc();
    }
}

fn as_rfc3339<S: Serializer>(time: &OffsetDateTime, serializer: S) -> Result<S::Ok, S::Error> {
    let text = time.format(&Rfc3339).map_err(serde::ser::Error::custom)?;
    serializer.serialize_str(&text)
}
