//! What the app hands a backend, and how a backend answers: one command in, any number of events
//! out, and a way to hear that the command was cancelled.

use std::sync::Arc;
use std::sync::mpsc::Sender;

use tokio::sync::watch;

use crate::contract::{Command, Event};

/// Where a backend sends the events that answer one command. It is `Send + Sync`: code that
/// reports progress captures `&Reply` and calls it between two awaits.
#[derive(Clone)]
pub struct Reply {
    events: Sender<Event>,
    wake: Arc<dyn Fn() + Send + Sync>,
    stop: watch::Receiver<bool>,
}

impl Reply {
    pub(super) fn new(
        events: Sender<Event>,
        wake: Arc<dyn Fn() + Send + Sync>,
        stop: watch::Receiver<bool>,
    ) -> Reply {
        Reply { events, wake, stop }
    }

    /// Sends the event, then wakes the window. It never waits: the channel has no bound.
    pub fn send(&self, event: Event) {
        // The window is gone when this fails, and then nobody waits for the event.
        if self.events.send(event).is_ok() {
            (self.wake)();
        }
    }

    /// Resolves when the app has cancelled this command, or the backend is gone. It stays
    /// true once the cancel came, even if the cancel came first, and it may be dropped and
    /// awaited again. Every task but an ingest is aborted and never sees it. An ingest is never
    /// aborted by the runtime: the ingest code drops it itself at a safe point and then sends
    /// `IngestFinished`.
    pub async fn cancelled(&self) {
        // An error here means the backend is gone, which also ends the wait.
        self.stop.clone().wait_for(|asked| *asked).await.ok();
    }

    /// For tests of a backend: the events arrive on the receiver, and
    /// `Stop::ask` is the cancel.
    #[cfg(feature = "testing")]
    pub fn collecting() -> (Reply, std::sync::mpsc::Receiver<Event>, Stop) {
        let (events, received) = std::sync::mpsc::channel();
        let (stop, stopped) = Stop::pair();
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
        (Reply::new(events, wake, stopped), received, stop)
    }
}

/// The other end of `Reply::cancelled`.
pub struct Stop {
    asked: watch::Sender<bool>,
}

impl Stop {
    pub(super) fn pair() -> (Stop, watch::Receiver<bool>) {
        let (asked, stopped) = watch::channel(false);
        (Stop { asked }, stopped)
    }

    pub fn ask(&self) {
        self.asked.send_replace(true);
    }
}

/// Does the work of one command and sends each result. The live backend and the fake backend
/// are these. It is named `Handler` because `contract::Service` is the health enum.
pub trait Handler: Send + Sync + 'static {
    fn serve(&self, command: Command, reply: Reply) -> impl Future<Output = ()> + Send;
}

// A later field that is not `Sync` would break the code that shares a reply between tasks,
// far from here, so it fails here.
const _: () = {
    const fn is_send_and_sync<T: Send + Sync>() {}
    is_send_and_sync::<Reply>();
};
