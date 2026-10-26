//! What the app hands a backend, and how a backend answers: one command in, any number of events
//! out, and a way to hear that the command was cancelled.

use std::sync::Arc;
use std::sync::mpsc::Sender;

use tokio::sync::watch;

use std::path::Path;

use contract::{Command, Event, Failure};

/// It is `Send + Sync`: code that reports progress captures `&Reply` and calls it between two
/// awaits.
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

    /// It never waits: the channel has no bound.
    pub fn send(&self, event: Event) {
        // The receiver is gone when this fails, as when the browser has left, and then nobody
        // waits for the event.
        if self.events.send(event).is_ok() {
            (self.wake)();
        }
    }

    /// Resolves when cancellation is requested or the runtime is gone; it can be awaited again.
    /// A cancel command asks an ingest to stop and aborts other requests. Dropping the runtime
    /// aborts all its tasks, including ingests.
    // SMELL: nothing calls this, so an ingest that is asked to stop runs to its end. Neither the
    // conversion nor the ingest has a point where it can stop safely, and only the converter can
    // give one between pages.
    pub async fn cancelled(&self) {
        // An error here means the backend is gone, which also ends the wait.
        self.stop.clone().wait_for(|asked| *asked).await.ok();
    }

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

/// Saves the model choice of one kind of work in the settings file of the home folder.
pub(crate) fn save_model_choice(
    home: &Path,
    work: choices::WorkKind,
    choice: choices::ModelSettings,
) -> Result<(), Failure> {
    let path = home.join(choices::MODEL_SETTINGS_FILE);
    choices::WorkChoices::load_from(&path)
        .and_then(|mut choices| {
            *choices.of_mut(work) = choice;
            choices.save_to(&path)
        })
        .map_err(|error| {
            Failure::internal(format!("could not save the model choice: {error}"))
                .with_hint(format!("The model choice could not be saved: {error}"))
        })
}

/// It is named `Handler` because `contract::Service` is the health enum.
pub trait Handler: Send + Sync + 'static {
    fn serve(&self, command: Command, reply: Reply) -> impl Future<Output = ()> + Send;
}

// A later field that is not `Sync` would break the code that shares a reply between tasks,
// far from here, so it fails here.
const _: () = {
    const fn is_send_and_sync<T: Send + Sync>() {}
    is_send_and_sync::<Reply>();
};
