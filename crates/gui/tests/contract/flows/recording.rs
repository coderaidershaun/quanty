//! A backend that writes down every command it is given and then lets another backend answer
//! it, so a test can say what the app sent.

use std::sync::{Arc, Mutex};

use gui::backend::fake::Fake;
use gui::backend::{Handler, Reply};
use gui::contract::{Command, StartupFacts};
use gui::testkit;

use super::Window;

#[derive(Clone, Default)]
pub(super) struct Seen(Arc<Mutex<Vec<Command>>>);

impl Seen {
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
    let home = testkit::samples_folder();
    let fake = Fake::scene(scene, &home)
        .unwrap_or_else(|error| panic!("the test cannot start the scene `{scene}`: {error}"))
        .instant();
    let opening = fake.opening();
    let facts = StartupFacts {
        home,
        env_file: None,
        fixture: Some(scene.to_owned()),
        anthropic_api_key_set: false,
    };
    let seen = Seen::default();
    let recording = Recording {
        inner: fake,
        seen: seen.clone(),
    };
    (testkit::app_on(recording, facts, opening, size), seen)
}
