//! A backend that writes down every command it is given and then lets another backend answer
//! it, so a test can say what the app sent.

use std::sync::{Arc, Mutex};

use backend::fake::Fake;
use backend::{Handler, Reply};
use contract::{Command, StartupFacts};
use gui::testkit;

use super::Window;

#[derive(Clone, Default)]
pub(super) struct Seen(Arc<Mutex<Vec<Command>>>, std::path::PathBuf);

impl Seen {
    pub(super) fn home(&self) -> &std::path::Path {
        &self.1
    }

    pub(super) fn all(&self) -> Vec<Command> {
        self.0
            .lock()
            .expect("the list of commands is not poisoned")
            .clone()
    }

    pub(super) fn count(&self, is: impl Fn(&Command) -> bool) -> usize {
        self.all().iter().filter(|command| is(command)).count()
    }
}

struct Recording<H> {
    inner: H,
    seen: Seen,
    _home: tempfile::TempDir,
}

impl<H: Handler> Handler for Recording<H> {
    async fn serve(&self, command: Command, reply: Reply) {
        self.seen
            .0
            .lock()
            .expect("the list of commands is not poisoned")
            .push(command.clone());
        self.inner.serve(command, reply).await;
    }
}

/// Nothing has run yet, so the start-up commands are not in the list until a frame runs.
pub(super) fn open(scene: &str, size: [f32; 2]) -> (Window, Seen) {
    open_with(scene, size, |fake| fake)
}

/// As `open`, with the fake wrapped by `wrap`. The list holds each command before `wrap` sees it.
pub(super) fn open_with<H: Handler>(
    scene: &str,
    size: [f32; 2],
    wrap: impl FnOnce(Fake) -> H,
) -> (Window, Seen) {
    let home = tempfile::tempdir().expect("a temporary fake home");
    let fake = Fake::scene(scene, home.path())
        .unwrap_or_else(|error| panic!("the test cannot start the scene `{scene}`: {error}"))
        .instant();
    let opening = fake.opening();
    let facts = StartupFacts {
        fixture: Some(scene.to_owned()),
        anthropic_api_key_set: false,
        model_choices: Default::default(),
        running: Vec::new(),
    };
    let seen = Seen(Arc::default(), home.path().to_owned());
    let recording = Recording {
        inner: wrap(fake),
        seen: seen.clone(),
        _home: home,
    };
    (testkit::app_on(recording, facts, opening, size), seen)
}
