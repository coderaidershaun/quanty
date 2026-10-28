//! The one slot of background work that the browser and the embedded MCP server of an HTTP
//! server share, and the kinds of work it can hold.

use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

/// Locks even when a holder panicked: the data these locks guard is never left half-written.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The background work occupying the one slot of an HTTP server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackgroundJob {
    pub kind: JobKind,
    pub name: String,
    id: Uuid,
}

impl fmt::Display for BackgroundJob {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} of {}", self.kind.label(), self.name)
    }
}

/// Admission shared by the browser and the embedded MCP server.
#[derive(Clone, Default)]
pub struct BackgroundJobs(Arc<Mutex<Option<BackgroundJob>>>);

impl BackgroundJobs {
    /// A new slot for one HTTP server.
    pub fn new() -> Self {
        Self::default()
    }

    /// The process slot used by browser routers that share a process.
    pub fn global() -> Self {
        static GLOBAL: OnceLock<BackgroundJobs> = OnceLock::new();
        GLOBAL.get_or_init(Self::new).clone()
    }

    /// The job still doing work, including work whose caller disconnected.
    pub fn running(&self) -> Option<BackgroundJob> {
        lock(&self.0).clone()
    }

    /// Holds admission until the returned reservation is dropped.
    ///
    /// # Errors
    /// Returns the running job when the slot is occupied.
    pub fn reserve(
        &self,
        kind: JobKind,
        name: impl Into<String>,
    ) -> Result<BackgroundReservation, BackgroundJob> {
        let mut slot = lock(&self.0);
        if let Some(running) = slot.as_ref() {
            return Err(running.clone());
        }
        let id = Uuid::new_v4();
        *slot = Some(BackgroundJob {
            kind,
            name: name.into(),
            id,
        });
        Ok(BackgroundReservation {
            jobs: self.clone(),
            id,
        })
    }
}

/// Ownership of one admitted background job.
pub struct BackgroundReservation {
    jobs: BackgroundJobs,
    id: Uuid,
}

impl Drop for BackgroundReservation {
    fn drop(&mut self) {
        let mut slot = lock(&self.jobs.0);
        if slot.as_ref().is_some_and(|running| running.id == self.id) {
            *slot = None;
        }
    }
}

/// The kind of work a job does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    /// A job that `ingest_pdf` started.
    Ingest,
    /// A job that `write_insights` started.
    Insights,
    /// A job that `build_dataset` started.
    Dataset,
}

impl JobKind {
    /// The short name shown for a running job.
    pub fn label(self) -> &'static str {
        match self {
            JobKind::Ingest => "ingest",
            JobKind::Insights => "insights",
            JobKind::Dataset => "dataset build",
        }
    }
}
