//! Paid work belongs to the server so closing its browser cannot abandon it.

use backend::{Handler, Runtime};
use contract::{Command, Failure, Frame, JobKind, RequestId, RunningJob};
use std::sync::{Arc, Mutex, OnceLock};
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

struct Active {
    id: Uuid,
    owner: Uuid,
    request: RequestId,
    job: RunningJob,
    cancel: mpsc::UnboundedSender<()>,
}

type Slot = Arc<Mutex<Option<Active>>>;
static GLOBAL: OnceLock<Slot> = OnceLock::new();

#[derive(Clone)]
pub(crate) struct Guard {
    slot: Slot,
}

impl Default for Guard {
    fn default() -> Self {
        Self {
            slot: GLOBAL.get_or_init(|| Arc::new(Mutex::new(None))).clone(),
        }
    }
}

struct Reservation {
    id: Uuid,
    slot: Slot,
}
impl Drop for Reservation {
    fn drop(&mut self) {
        let mut slot = self.slot.lock().unwrap_or_else(|e| e.into_inner());
        if slot.as_ref().is_some_and(|active| active.id == self.id) {
            *slot = None;
        }
    }
}

impl Guard {
    pub(crate) fn running(&self) -> Vec<RunningJob> {
        self.slot
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|active| vec![active.job.clone()])
            .unwrap_or_default()
    }

    pub(crate) fn cancel(&self, owner: &Owner, request: RequestId) -> bool {
        let slot = self.slot.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(active) = slot
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
        let job = match &command {
            Command::Ingest { ingest, .. } => RunningJob {
                kind: JobKind::Ingest,
                name: ingest.file_name.clone(),
            },
            Command::BuildDataset { build, .. } => RunningJob {
                kind: JobKind::Dataset,
                name: build.name.clone(),
            },
            _ => unreachable!("only paid commands take the job slot"),
        };
        let request = command.request();
        let id = Uuid::new_v4();
        let (cancel, mut cancelled) = mpsc::unbounded_channel();
        {
            let mut slot = self.slot.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(active) = slot.as_ref() {
                return Err(Failure::internal("an ingest is running").with_hint(format!(
                    "An ingest of {} is already running from another window. Wait for it to end.",
                    active.job.name
                )));
            }
            *slot = Some(Active {
                id,
                owner: owner.id,
                request,
                job,
                cancel,
            });
        }
        let frames = owner.frames;
        let reservation = Reservation {
            id,
            slot: self.slot.clone(),
        };
        tokio::spawn(async move {
            let wake = Arc::new(Notify::new());
            let notify = wake.clone();
            let mut runtime = Runtime::on(tokio::runtime::Handle::current(), handler, move || {
                notify.notify_one()
            });
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
