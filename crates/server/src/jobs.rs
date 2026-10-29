//! Paid work belongs to the server so closing its browser cannot abandon it.

use backend::{Handler, Runtime};
use contract::{
    Command, Event, Failure, Frame, IngestOutcome, IngestProgress, IngestRequest, RequestId,
    RunningJob,
};
use mcp::{BackgroundJobs, JobKind, LibraryChanges};
use std::sync::{Arc, Mutex, MutexGuard};
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

/// A tab that gets the events of the running job, under the request id it knows the job by.
struct Follower {
    owner: Uuid,
    request: RequestId,
    frames: mpsc::UnboundedSender<Frame>,
}

/// What a tab that arrives late needs to know of the ingest that runs.
struct WatchedIngest {
    ingest: IngestRequest,
    progress: Option<IngestProgress>,
    result: Option<Result<IngestOutcome, Failure>>,
    /// Whether a tab got `IngestFinished`.
    is_end_seen: bool,
}

struct Active {
    id: Uuid,
    /// The starter comes first and is the one that the job's own request id belongs to.
    followers: Vec<Follower>,
    cancel: mpsc::UnboundedSender<()>,
    /// Only an ingest is followed by tabs that did not start it.
    ingest: Option<WatchedIngest>,
}

/// An ingest that ended while no tab was there to see its end.
struct UnseenEnd {
    ingest: IngestRequest,
    progress: Option<IngestProgress>,
    result: Result<IngestOutcome, Failure>,
}

#[derive(Default)]
struct State {
    active: Option<Active>,
    unseen_end: Option<UnseenEnd>,
}

fn locked(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state.lock().unwrap_or_else(|e| e.into_inner())
}

impl State {
    /// Sends the event to every follower and forgets the followers whose tab has gone. The paid
    /// work goes on when no tab is left.
    fn deliver(&mut self, event: &Event) {
        let Some(active) = &mut self.active else {
            return;
        };
        if let Some(watched) = &mut active.ingest {
            match event {
                Event::IngestProgress { progress, .. } => watched.progress = Some(progress.clone()),
                Event::IngestFinished { result, .. } => watched.result = Some(result.clone()),
                _ => {}
            }
        }
        active.followers.retain(|follower| {
            let frame = Frame::Event(addressed_to(event, follower.request));
            follower.frames.send(frame).is_ok()
        });
        if matches!(event, Event::IngestFinished { .. })
            && let Some(watched) = &mut active.ingest
        {
            watched.is_end_seen = !active.followers.is_empty();
        }
    }

    /// Ends the job: every tab that still follows it hears that it is over, and the end of an
    /// ingest that no tab saw is kept for the next tab that asks.
    fn end(&mut self) {
        let Some(active) = self.active.take() else {
            return;
        };
        for follower in &active.followers {
            if follower
                .frames
                .send(Frame::Finished(follower.request))
                .is_err()
            {
                tracing::trace!(request = ?follower.request, "paid completion has no browser");
            }
        }
        if let Some(watched) = active.ingest
            && !watched.is_end_seen
            && let Some(result) = watched.result
        {
            self.unseen_end = Some(UnseenEnd {
                ingest: watched.ingest,
                progress: watched.progress,
                result,
            });
        }
    }
}

/// The ingest events carry the request id of the tab that started the job, and a tab that follows
/// it knows the job by its own.
fn addressed_to(event: &Event, request: RequestId) -> Event {
    match event {
        Event::IngestProgress { progress, .. } => Event::IngestProgress {
            request,
            progress: progress.clone(),
        },
        Event::IngestFinished { result, .. } => Event::IngestFinished {
            request,
            result: result.clone(),
        },
        other => other.clone(),
    }
}

#[derive(Clone)]
pub(crate) struct Guard {
    jobs: BackgroundJobs,
    library_changes: LibraryChanges,
    state: Arc<Mutex<State>>,
}

impl Guard {
    pub(crate) fn new(jobs: BackgroundJobs, library_changes: LibraryChanges) -> Self {
        Self {
            jobs,
            library_changes,
            state: Arc::new(Mutex::new(State::default())),
        }
    }

    /// Tells the agents that the browser changed the library.
    pub(crate) fn library_changed(&self) {
        self.library_changes.notify();
    }
}

/// Clears the job if its task dies before it has ended it.
struct ActiveGuard {
    id: Uuid,
    state: Arc<Mutex<State>>,
}
impl Drop for ActiveGuard {
    fn drop(&mut self) {
        let mut state = locked(&self.state);
        if state.active.as_ref().is_some_and(|job| job.id == self.id) {
            state.active = None;
        }
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
        let state = locked(&self.state);
        if let Some(active) = state.active.as_ref().filter(|active| {
            active
                .followers
                .iter()
                .any(|follower| follower.owner == owner.id && follower.request == request)
        }) {
            if active.cancel.send(()).is_err() {
                tracing::debug!(?request, "the paid task already ended");
            }
            return true;
        }
        false
    }

    /// Gives the tab the ingest that runs, so a reloaded tab shows it again and may stop it.
    /// An ingest that ended while no tab was there is given once, with its end. With no ingest,
    /// the tab hears only that the request is over.
    // SMELL: only an ingest that a browser tab started is kept here. An ingest that an agent
    // started is not shown, and the tab only says that background work runs.
    pub(crate) fn follow(&self, owner: &Owner, request: RequestId) {
        let mut state = locked(&self.state);
        let State { active, unseen_end } = &mut *state;
        let send = |event| owner.frames.send(Frame::Event(event)).is_ok();
        if let Some(active) = active
            && let Some(watched) = &mut active.ingest
        {
            let followed = Event::IngestFollowed {
                request,
                ingest: watched.ingest.clone(),
                progress: watched.progress.clone(),
            };
            if !send(followed) {
                return;
            }
            // The end goes out a moment before the job is closed, and a tab that comes in between
            // must still get it.
            if let Some(result) = &watched.result {
                send(Event::IngestFinished {
                    request,
                    result: result.clone(),
                });
                watched.is_end_seen = true;
            }
            active.followers.push(Follower {
                owner: owner.id,
                request,
                frames: owner.frames.clone(),
            });
            return;
        }
        if let Some(ended) = unseen_end.take() {
            send(Event::IngestFollowed {
                request,
                ingest: ended.ingest,
                progress: ended.progress,
            });
            send(Event::IngestFinished {
                request,
                result: ended.result,
            });
        }
        if owner.frames.send(Frame::Finished(request)).is_err() {
            tracing::trace!(?request, "the follow has no browser");
        }
    }

    pub(crate) fn start<H: Handler>(
        &self,
        owner: Owner,
        command: Command,
        handler: Arc<H>,
    ) -> Result<(), Failure> {
        let (kind, name, watched) = match &command {
            Command::Ingest { ingest, .. } => (
                JobKind::Ingest,
                ingest.file_name.clone(),
                Some(WatchedIngest {
                    ingest: ingest.clone(),
                    progress: None,
                    result: None,
                    is_end_seen: false,
                }),
            ),
            Command::BuildDataset { build, .. } => (JobKind::Dataset, build.name.clone(), None),
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
            let mut state = locked(&self.state);
            state.unseen_end = None;
            state.active = Some(Active {
                id,
                followers: vec![Follower {
                    owner: owner.id,
                    request,
                    frames: owner.frames,
                }],
                cancel,
                ingest: watched,
            });
        }
        let state = self.state.clone();
        let library_changes = self.library_changes.clone();
        let guard = ActiveGuard {
            id,
            state: state.clone(),
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
                    locked(&state).deliver(&event);
                }
                if !finished.is_empty() {
                    {
                        // The slot is freed under the lock, so a new start cannot set its job
                        // before this one is closed.
                        let mut state = locked(&state);
                        drop(admission);
                        state.end();
                    }
                    if kind == JobKind::Ingest {
                        library_changes.notify();
                    }
                    break;
                }
            }
            drop(guard);
        });
        Ok(())
    }
}
