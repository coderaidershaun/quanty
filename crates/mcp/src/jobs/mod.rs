//! The list of the background jobs of one kind, how one job runs, how a running job is stopped,
//! and how a caller follows it. A job is one `watch` channel that carries its report: only the
//! tasks of the job write it, and any number of callers read it.

use std::collections::VecDeque;
use std::future::Future;
use std::sync::Mutex;

use tokio::sync::watch;
use tokio::task::{AbortHandle, JoinError};
use uuid::Uuid;

mod background;
mod list;
mod stop;
mod view;
mod wait;

pub(crate) use background::lock;
pub use background::{BackgroundJob, BackgroundJobs, BackgroundReservation, JobKind};
pub(crate) use list::JobList;
pub(crate) use stop::{StopError, StopJobArgs, StopResult, StopState};
pub(crate) use view::{Job, JobState, Stage, rfc3339};
pub(crate) use wait::{ProgressSink, WatchError, WatchJobArgs, wait_for_change};

/// The report of a job: the [`Job`] that every kind has, and the fields of its own kind.
pub(crate) trait JobReport: Clone + Send + Sync + 'static {
    fn job(&self) -> &Job;

    /// The end of a job that `stop_job` stopped. The report says what the person does to go on.
    fn stopped(&mut self);

    fn is_running(&self) -> bool {
        self.job().state() == JobState::Running
    }

    /// Whether the report is the end that [`JobReport::stopped`] wrote.
    fn is_stopped(&self) -> bool {
        self.job().state() == JobState::Stopped
    }
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

/// Makes `Busy` and `BackgroundJob` convertible into the variants of a tool error that hold them.
macro_rules! busy_from {
    ($error:ident) => {
        impl From<Busy> for $error {
            fn from(busy: Busy) -> $error {
                $error::Busy(busy)
            }
        }

        impl From<BackgroundJob> for $error {
            fn from(job: BackgroundJob) -> $error {
                $error::SharedBusy(job)
            }
        }
    };
}
pub(crate) use busy_from;

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

/// The newest jobs of one kind, oldest first, and the shared slot that admits them.
pub(crate) struct Jobs<R> {
    kind: JobKind,
    background: Option<BackgroundJobs>,
    list: Mutex<VecDeque<Listed<R>>>,
}

impl<R: JobReport> Jobs<R> {
    pub(crate) fn new(kind: JobKind, background: Option<BackgroundJobs>) -> Jobs<R> {
        Jobs {
            kind,
            background,
            list: Mutex::default(),
        }
    }

    /// Takes the shared slot for a job named `name` when the server has one.
    ///
    /// # Errors
    /// The job that holds the slot.
    pub(crate) fn reserve(
        &self,
        name: impl FnOnce() -> String,
    ) -> Result<Option<BackgroundReservation>, BackgroundJob> {
        self.background
            .as_ref()
            .map(|slot| slot.reserve(self.kind, name()))
            .transpose()
    }

    /// Makes a job, unless one is running, and forgets the oldest job when the list is full. The
    /// check and the new job are in one lock with no `.await` between them, so two calls at the
    /// same moment cannot both start. It holds in one server only: a second server, or a command
    /// such as `rag-ingest pdf` or `rag-dataset build`, is not stopped from doing the same work at
    /// the same time.
    ///
    /// # Errors
    /// [`Busy`] with the id of the job that is running.
    pub(crate) fn start(&self, first_report: impl FnOnce(Uuid) -> R) -> Result<Started<R>, Busy> {
        let mut list = lock(&self.list);
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
        self.watcher_of(job_id)
            .map(|watcher| watcher.borrow().clone())
    }

    /// A reader of the reports of the job with this id, or `None` when no job has it.
    pub(crate) fn watcher_of(&self, job_id: &str) -> Option<watch::Receiver<R>> {
        let wanted = Uuid::parse_str(job_id.trim()).ok()?;
        let list = lock(&self.list);
        list.iter()
            .find(|listed| listed.id == wanted)
            .map(|listed| listed.watcher.clone())
    }

    /// The [`Job`] of the newest `limit` jobs, newest first.
    pub(crate) fn newest(&self, limit: usize) -> Vec<Job> {
        let list = lock(&self.list);
        list.iter()
            .rev()
            .take(limit)
            .map(|listed| listed.watcher.borrow().job().clone())
            .collect()
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
        let mut list = lock(&self.list);
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
            let list = lock(&self.list);
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

/// The first report that `ready` accepts. `None` when the tasks of the job are gone before one
/// was written.
pub(crate) async fn first_report<R: JobReport>(
    mut watcher: watch::Receiver<R>,
    ready: impl FnMut(&R) -> bool,
) -> Option<R> {
    let report = watcher.wait_for(ready).await.ok()?;
    Some(report.clone())
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
        job: Job,
        running: bool,
        stopped: bool,
        admission: BackgroundJobs,
    }

    impl JobReport for Report {
        fn job(&self) -> &Job {
            &self.job
        }
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
            job: Job::running(Uuid::new_v4(), JobKind::Ingest),
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
        let jobs = Jobs::<Report>::new(JobKind::Ingest, None);
        let second_kind = Jobs::<Report>::new(JobKind::Ingest, None);
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
