//! Paid work belongs to the server so closing its browser cannot abandon it.

use backend::{Handler, Runtime};
use contract::{Command, Failure, Frame, RequestId, RunningJob};
use mcp::{BackgroundJobs, BackgroundReservation, JobKind};
use std::sync::{Arc, Mutex};
use tokio::sync::{Notify, mpsc};
use uuid::Uuid;

#[derive(Clone)]
pub(crate) struct Owner {
    id: Uuid,
    frames: mpsc::UnboundedSender<Frame>,
}

impl Owner {
    pub(crate) fn new(frames: mpsc::UnboundedSender<Frame>) -> Self {
        Self {
            id: Uuid::new_v4(),
            frames,
        }
    }
}

/// A runtime for `handler`, and the signal that tells its owner there is something to collect.
pub(crate) fn spawn_runtime<H: Handler>(handler: Arc<H>) -> (Runtime<H>, Arc<Notify>) {
    let wake = Arc::new(Notify::new());
    let notify = wake.clone();
    let runtime = Runtime::on(tokio::runtime::Handle::current(), handler, move || {
        notify.notify_one()
    });
    (runtime, wake)
}

struct Active {
    id: Uuid,
    owner: Uuid,
    request: RequestId,
    cancel: mpsc::UnboundedSender<()>,
}

#[derive(Clone)]
pub(crate) struct Guard {
    jobs: BackgroundJobs,
    active: Arc<Mutex<Option<Active>>>,
}

impl Guard {
    pub(crate) fn new(jobs: BackgroundJobs) -> Self {
        Self {
            jobs,
            active: Arc::new(Mutex::new(None)),
        }
    }
}

struct OwnedReservation {
    id: Uuid,
    active: Arc<Mutex<Option<Active>>>,
    _admission: BackgroundReservation,
}
impl Drop for OwnedReservation {
    fn drop(&mut self) {
        let mut active = self.active.lock().unwrap_or_else(|e| e.into_inner());
        if active.as_ref().is_some_and(|job| job.id == self.id) {
            *active = None;
        }
        drop(active);
    }
}

impl Guard {
    pub(crate) fn running(&self) -> Vec<RunningJob> {
        self.jobs
            .running()
            .map(|job| {
                vec![RunningJob {
                    kind: match job.kind {
                        JobKind::Ingest => contract::JobKind::Ingest,
                        JobKind::Dataset => contract::JobKind::Dataset,
                        JobKind::Insights => contract::JobKind::Insights,
                    },
                    name: job.name,
                }]
            })
            .unwrap_or_default()
    }

    pub(crate) fn cancel(&self, owner: &Owner, request: RequestId) -> bool {
        let active = self.active.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(active) = active
            .as_ref()
            .filter(|active| active.owner == owner.id && active.request == request)
        {
            if active.cancel.send(()).is_err() {
                tracing::debug!(?request, "the paid task already ended");
            }
            return true;
        }
        false
    }

    pub(crate) fn start<H: Handler>(
        &self,
        owner: Owner,
        command: Command,
        handler: Arc<H>,
    ) -> Result<(), Failure> {
        let (kind, name) = match &command {
            Command::Ingest { ingest, .. } => (JobKind::Ingest, ingest.file_name.clone()),
            Command::BuildDataset { build, .. } => (JobKind::Dataset, build.name.clone()),
            _ => unreachable!("only paid commands take the job slot"),
        };
        let request = command.request();
        let id = Uuid::new_v4();
        let (cancel, mut cancelled) = mpsc::unbounded_channel();
        let admission = self.jobs.reserve(kind, name).map_err(|running| {
            Failure::internal("an ingest is running").with_hint(format!(
                "Background work is already running: {} of {}. Wait for it to end, then reload this page.",
                running.kind.label(),
                running.name
            ))
        })?;
        {
            let mut active = self.active.lock().unwrap_or_else(|e| e.into_inner());
            *active = Some(Active {
                id,
                owner: owner.id,
                request,
                cancel,
            });
        }
        let frames = owner.frames;
        let reservation = OwnedReservation {
            id,
            active: self.active.clone(),
            _admission: admission,
        };
        tokio::spawn(async move {
            let (mut runtime, wake) = spawn_runtime(handler);
            runtime.send(command);
            loop {
                tokio::select! {
                    _ = wake.notified() => {},
                    Some(()) = cancelled.recv() => runtime.send(Command::Cancel(request)),
                }
                // Completion can arrive while events are drained, so collect it first.
                let finished = runtime.take_finished();
                while let Some(event) = runtime.try_next() {
                    // A closed browser stops receiving progress, but its paid work continues.
                    if frames.send(Frame::Event(event)).is_err() {
                        tracing::trace!(?request, "paid progress has no browser");
                    }
                }
                if !finished.is_empty() {
                    drop(reservation);
                    for request in finished {
                        if frames.send(Frame::Finished(request)).is_err() {
                            tracing::trace!(?request, "paid completion has no browser");
                        }
                    }
                    break;
                }
            }
        });
        Ok(())
    }
}
