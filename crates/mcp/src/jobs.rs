//! The list of the background jobs of one kind, how one job runs, and how a running job is
//! stopped. A job is one `watch` channel that carries its report: only the tasks of the job write
//! it, and any number of callers read it.

use std::collections::VecDeque;
use std::future::Future;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use tokio::task::{AbortHandle, JoinError};
use uuid::Uuid;

/// A report that says whether its job is still going on.
pub(crate) trait JobReport: Clone + Send + Sync + 'static {
    fn is_running(&self) -> bool;

    /// The end of a job that `stop_job` stopped. The report says what the person does to go on.
    fn stopped(&mut self);

    /// Whether the report is the end that [`JobReport::stopped`] wrote.
    fn is_stopped(&self) -> bool;
}

/// What the tasks of a job get when it starts.
pub(crate) struct Started<R> {
    pub(crate) id: Uuid,
    /// The one place a report is written. The tasks of the job hold clones of it.
    pub(crate) report: watch::Sender<R>,
    pub(crate) watcher: watch::Receiver<R>,
}

/// The job that is running, which is why a new one cannot start.
#[derive(Debug)]
pub(crate) struct Busy {
    pub(crate) running_job: Uuid,
}

/// The background work occupying the one slot of an HTTP server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackgroundJob {
    pub kind: JobKind,
    pub name: String,
    id: Uuid,
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
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
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
        let mut slot = self.0.lock().unwrap_or_else(PoisonError::into_inner);
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
        let mut slot = self.jobs.0.lock().unwrap_or_else(PoisonError::into_inner);
        if slot.as_ref().is_some_and(|running| running.id == self.id) {
            *slot = None;
        }
    }
}

/// What an agent gives `stop_job`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct StopJobArgs {
    /// The `job_id` that `ingest_pdf`, `write_insights` or `build_dataset` gave.
    pub(crate) job_id: String,
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

    /// The tool that tells how a job of this kind goes or how it ended.
    fn status_tool(self) -> &'static str {
        match self {
            JobKind::Ingest => "ingest_status",
            JobKind::Insights => "insights_status",
            JobKind::Dataset => "dataset_status",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StopState {
    /// The job was running and is stopped now.
    Stopped,
    /// The job had ended already, and was left as it was.
    Ended,
}

/// What `stop_job` did.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct StopResult {
    job_id: String,
    kind: JobKind,
    state: StopState,
    /// The tool that says how the job ended and what to do next.
    status_tool: &'static str,
}

impl StopResult {
    pub(crate) fn new(job_id: &str, kind: JobKind, state: StopState) -> StopResult {
        StopResult {
            job_id: job_id.trim().to_owned(),
            kind,
            state,
            status_tool: kind.status_tool(),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum StopError {
    #[error(
        "no job of this server has the id {job_id:?}; job ids are forgotten when the server restarts, and the server keeps only its last {KEPT_JOBS} jobs of each kind"
    )]
    UnknownJob { job_id: String },
}

/// How many jobs the list keeps. Without a limit, a server that runs for months would keep one
/// report for each job it was ever given.
pub(crate) const KEPT_JOBS: usize = 100;

/// One job of the list.
struct Listed<R> {
    id: Uuid,
    watcher: watch::Receiver<R>,
    /// Set when the work starts. A caller learns the id of a job only after that, so a running
    /// job that a caller names always has it.
    abort: Option<AbortHandle>,
}

/// The newest jobs of one kind, oldest first.
pub(crate) struct Jobs<R> {
    list: Mutex<VecDeque<Listed<R>>>,
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
        if let Some(running) = list.iter().find(|listed| is_running(&listed.watcher)) {
            return Err(Busy {
                running_job: running.id,
            });
        }
        // No job is running here, so the job that is forgotten has ended.
        if list.len() >= KEPT_JOBS {
            list.pop_front();
        }
        let id = Uuid::new_v4();
        let (report, watcher) = watch::channel(first_report(id));
        list.push_back(Listed {
            id,
            watcher: watcher.clone(),
            abort: None,
        });
        Ok(Started {
            id,
            report,
            watcher,
        })
    }

    /// The latest report of the job with this id, or `None` when no job has it.
    pub(crate) fn report_of(&self, job_id: &str) -> Option<R> {
        let wanted = Uuid::parse_str(job_id.trim()).ok()?;
        let list = self.list.lock().unwrap_or_else(PoisonError::into_inner);
        list.iter()
            .find(|listed| listed.id == wanted)
            .map(|listed| listed.watcher.borrow().clone())
    }

    /// Runs `work` for the job `id` on a task of its own, so that it goes on after the tool call
    /// ends, and writes how it ended into the report with `end`. The task that waits for the work
    /// must not panic: without the end the report of the job would say `running` for ever. A work
    /// task that panicked ends as the `E` made from its `JoinError`; one that [`Jobs::stop`]
    /// aborted ends as [`JobReport::stopped`].
    pub(crate) fn spawn<T, E>(
        &self,
        id: Uuid,
        report: watch::Sender<R>,
        work: impl Future<Output = Result<T, E>> + Send + 'static,
        end: impl FnOnce(&mut R, Result<T, E>) + Send + 'static,
        reservation: Option<BackgroundReservation>,
    ) where
        T: Send + 'static,
        E: From<JoinError> + Send + 'static,
    {
        let running = tokio::spawn(work);
        let mut list = self.list.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(listed) = list.iter_mut().find(|listed| listed.id == id) {
            listed.abort = Some(running.abort_handle());
        }
        drop(list);
        tokio::spawn(async move {
            match running.await {
                Err(aborted) if aborted.is_cancelled() => report.send_modify(R::stopped),
                ended => {
                    let ended = ended.unwrap_or_else(|panicked| Err(E::from(panicked)));
                    report.send_modify(|report| end(report, ended));
                }
            }
            drop(reservation);
        });
    }

    /// Aborts the job with this id when it is running, and answers once its report says it is not.
    /// `None` when no job has the id. Aborting ends the work at its next wait, and every process
    /// it started goes with it. A job that ended by itself
    /// just before the abort could reach it answers `Ended`, as its report says.
    pub(crate) async fn stop(&self, job_id: &str) -> Option<StopState> {
        let wanted = Uuid::parse_str(job_id.trim()).ok()?;
        let (mut watcher, abort) = {
            let list = self.list.lock().unwrap_or_else(PoisonError::into_inner);
            let listed = list.iter().find(|listed| listed.id == wanted)?;
            (listed.watcher.clone(), listed.abort.clone())
        };
        if !is_running(&watcher) {
            return Some(StopState::Ended);
        }
        if let Some(abort) = abort {
            abort.abort();
        }
        // The waiter of the job writes the end. A report whose writers are gone has ended too.
        let _ = watcher.wait_for(|report| !report.is_running()).await;
        let stopped = watcher.borrow().is_stopped();
        Some(if stopped {
            StopState::Stopped
        } else {
            StopState::Ended
        })
    }
}

/// Whether the job is going on. A job whose tasks are gone is not, whatever its report says:
/// otherwise a job that lost its tasks before the end was written would refuse every later job.
fn is_running<R: JobReport>(watcher: &watch::Receiver<R>) -> bool {
    // `has_changed` fails only when every writer of the report is gone.
    watcher.borrow().is_running() && watcher.has_changed().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::pending;
    use std::time::Duration;
    use tokio::sync::oneshot;

    #[derive(Clone)]
    struct Report {
        running: bool,
        stopped: bool,
        admission: BackgroundJobs,
    }

    impl JobReport for Report {
        fn is_running(&self) -> bool {
            self.running
        }
        fn stopped(&mut self) {
            assert!(
                self.admission.running().is_some(),
                "admission ended before the stopped report"
            );
            self.running = false;
            self.stopped = true;
        }
        fn is_stopped(&self) -> bool {
            self.stopped
        }
    }

    fn report(admission: BackgroundJobs) -> Report {
        Report {
            running: true,
            stopped: false,
            admission,
        }
    }

    async fn released(jobs: &BackgroundJobs) {
        tokio::time::timeout(Duration::from_secs(5), async {
            while jobs.running().is_some() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }

    async fn settled(watcher: &mut watch::Receiver<Report>, admission: &BackgroundJobs) {
        tokio::time::timeout(
            Duration::from_secs(5),
            watcher.wait_for(|report| !report.running),
        )
        .await
        .unwrap()
        .unwrap();
        released(admission).await;
    }

    struct DropProbe(BackgroundJobs);

    impl Drop for DropProbe {
        fn drop(&mut self) {
            assert!(
                self.0.running().is_some(),
                "admission ended before the work was dropped"
            );
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn background_work_holds_admission_until_it_is_settled() {
        let admission = BackgroundJobs::new();
        let jobs = Jobs::<Report>::default();
        let second_kind = Jobs::<Report>::default();
        let first = jobs.start(|_| report(BackgroundJobs::new())).unwrap();
        let second = second_kind
            .start(|_| report(BackgroundJobs::new()))
            .unwrap();
        assert_ne!(
            first.id, second.id,
            "standalone lists admit different kinds together"
        );
        drop(first);
        drop(second);

        let reservation = admission.reserve(JobKind::Ingest, "normal").unwrap();
        let started = jobs.start(|_| report(admission.clone())).unwrap();
        let mut watcher = started.watcher.clone();
        let (release, held) = oneshot::channel();
        let settling = admission.clone();
        jobs.spawn(
            started.id,
            started.report.clone(),
            async move {
                held.await.unwrap();
                Ok::<(), JoinError>(())
            },
            move |report, _| {
                assert!(
                    settling.running().is_some(),
                    "admission ended before the final report"
                );
                report.running = false;
            },
            Some(reservation),
        );
        drop(started);
        assert_eq!(admission.running().unwrap().name, "normal");
        assert!(admission.reserve(JobKind::Dataset, "next").is_err());
        release.send(()).unwrap();
        settled(&mut watcher, &admission).await;

        let reservation = admission.reserve(JobKind::Insights, "panic").unwrap();
        let started = jobs.start(|_| report(admission.clone())).unwrap();
        let mut watcher = started.watcher.clone();
        let (release, held) = oneshot::channel();
        let settling = admission.clone();
        jobs.spawn::<(), JoinError>(
            started.id,
            started.report.clone(),
            async move {
                held.await.unwrap();
                panic!("controlled work panic")
            },
            move |report, result| {
                assert!(
                    settling.running().is_some(),
                    "admission ended before the failed report"
                );
                assert!(result.is_err());
                report.running = false;
            },
            Some(reservation),
        );
        drop(started);
        assert_eq!(admission.running().unwrap().kind, JobKind::Insights);
        release.send(()).unwrap();
        settled(&mut watcher, &admission).await;

        let reservation = admission.reserve(JobKind::Dataset, "stopped").unwrap();
        let started = jobs.start(|_| report(admission.clone())).unwrap();
        let job_id = started.id.to_string();
        let (entered, reached) = oneshot::channel();
        let probe = DropProbe(admission.clone());
        jobs.spawn(
            started.id,
            started.report.clone(),
            async move {
                let _probe = probe;
                entered.send(()).unwrap();
                pending::<Result<(), JoinError>>().await
            },
            |report, _| report.running = false,
            Some(reservation),
        );
        drop(started);
        tokio::time::timeout(Duration::from_secs(5), reached)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(admission.running().unwrap().name, "stopped");
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), jobs.stop(&job_id))
                .await
                .unwrap(),
            Some(StopState::Stopped)
        );
        released(&admission).await;
    }
}
