//! The list of the background jobs of one kind, and how one job runs. A job is one `watch` channel
//! that carries its report: only the tasks of the job write it, and any number of callers read it.

use std::collections::VecDeque;
use std::future::Future;
use std::sync::{Mutex, PoisonError};

use tokio::sync::watch;
use tokio::task::JoinError;
use uuid::Uuid;

/// A report that says whether its job is still going on.
pub(crate) trait JobReport: Clone + Send + Sync + 'static {
    fn is_running(&self) -> bool;
}

/// What the tasks of a job get when it starts.
pub(crate) struct Started<R> {
    /// The one place a report is written. The tasks of the job hold clones of it.
    pub(crate) report: watch::Sender<R>,
    pub(crate) watcher: watch::Receiver<R>,
}

/// The job that is running, which is why a new one cannot start.
#[derive(Debug)]
pub(crate) struct Busy {
    pub(crate) running_job: Uuid,
}

/// How many jobs the list keeps. Without a limit, a server that runs for months would keep one
/// report for each job it was ever given.
pub(crate) const KEPT_JOBS: usize = 100;

/// The newest jobs of one kind, oldest first.
pub(crate) struct Jobs<R> {
    list: Mutex<VecDeque<(Uuid, watch::Receiver<R>)>>,
}

// Written by hand, because `#[derive(Default)]` would require `R: Default`.
impl<R> Default for Jobs<R> {
    fn default() -> Jobs<R> {
        Jobs {
            list: Mutex::new(VecDeque::new()),
        }
    }
}

impl<R: JobReport> Jobs<R> {
    /// Makes a job, unless one is running, and forgets the oldest job when the list is full. The
    /// check and the new job are in one lock with no `.await` between them, so two calls at the
    /// same moment cannot both start. It holds in one server only: a second server, or a command
    /// such as `rag-ingest pdf` or `rag-dataset build`, is not stopped from doing the same work at
    /// the same time.
    ///
    /// # Errors
    /// [`Busy`] with the id of the job that is running.
    pub(crate) fn start(&self, first_report: impl FnOnce(Uuid) -> R) -> Result<Started<R>, Busy> {
        let mut list = self.list.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((running, _)) = list.iter().find(|(_, watcher)| is_running(watcher)) {
            return Err(Busy {
                running_job: *running,
            });
        }
        // No job is running here, so the job that is forgotten has ended.
        if list.len() >= KEPT_JOBS {
            list.pop_front();
        }
        let id = Uuid::new_v4();
        let (report, watcher) = watch::channel(first_report(id));
        list.push_back((id, watcher.clone()));
        Ok(Started { report, watcher })
    }

    /// The latest report of the job with this id, or `None` when no job has it.
    pub(crate) fn report_of(&self, job_id: &str) -> Option<R> {
        let wanted = Uuid::parse_str(job_id.trim()).ok()?;
        let list = self.list.lock().unwrap_or_else(PoisonError::into_inner);
        list.iter()
            .find(|(id, _)| *id == wanted)
            .map(|(_, watcher)| watcher.borrow().clone())
    }
}

/// Whether the job is going on. A job whose tasks are gone is not, whatever its report says:
/// otherwise a job that lost its tasks before the end was written would refuse every later job.
fn is_running<R: JobReport>(watcher: &watch::Receiver<R>) -> bool {
    // `has_changed` fails only when every writer of the report is gone.
    watcher.borrow().is_running() && watcher.has_changed().is_ok()
}

/// Runs `work` on a task of its own, so that it goes on after the tool call ends, and writes how
/// it ended into the report with `end`. The task that waits for the work must not panic: without
/// the end the report of the job would say `running` for ever. A work task that panicked or was
/// cancelled ends as the `E` made from its `JoinError`.
pub(crate) fn spawn<R, T, E>(
    report: watch::Sender<R>,
    work: impl Future<Output = Result<T, E>> + Send + 'static,
    end: impl FnOnce(&mut R, Result<T, E>) + Send + 'static,
) where
    R: Send + Sync + 'static,
    T: Send + 'static,
    E: From<JoinError> + Send + 'static,
{
    let running = tokio::spawn(work);
    tokio::spawn(async move {
        let ended = running
            .await
            .unwrap_or_else(|stopped| Err(E::from(stopped)));
        report.send_modify(|report| end(report, ended));
    });
}
