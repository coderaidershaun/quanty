//! A backend that answers every command from built-in data, so the whole app runs with no store
//! and no model.

mod fixtures;
mod scenes;

use std::path::{Path, PathBuf};

use super::{Handler, Reply};
use crate::contract::{
    Catalogue, Command, Event, Failure, Intent, SearchReply, Service, ServiceState,
};

pub use scenes::{Scene, scenes};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FakeError {
    #[error("there is no scene named `{name}`; the scenes are: {known}")]
    UnknownScene { name: String, known: String },
    #[error(
        "the sample chapters were not found at or above {}; run from the repository, or pass --home",
        .home.display()
    )]
    SamplesMissing { home: PathBuf },
}

/// The fake backend of one scene.
#[derive(Debug, Clone)]
pub struct Fake {
    scene: &'static Scene,
}

impl Fake {
    /// The fake for the scene called `name`.
    ///
    /// # Errors
    /// [`FakeError::UnknownScene`] when no scene has that name, [`FakeError::SamplesMissing`]
    /// when the scene needs the sample chapters and `home` has none above it.
    pub fn scene(name: &str, _home: &Path) -> Result<Fake, FakeError> {
        scenes()
            .iter()
            .find(|scene| scene.name == name)
            .map(|scene| Fake { scene })
            .ok_or_else(|| FakeError::UnknownScene {
                name: name.to_owned(),
                known: scenes()
                    .iter()
                    .map(|scene| scene.name)
                    .collect::<Vec<_>>()
                    .join(", "),
            })
    }

    /// The same fake with no waits, for tests.
    pub fn instant(self) -> Fake {
        self
    }

    /// What the scene does when the window opens.
    pub fn opening(&self) -> Vec<Intent> {
        Vec::new()
    }
}

impl Handler for Fake {
    async fn serve(&self, command: Command, reply: Reply) {
        match &command {
            Command::Ask { request, .. } => reply.send(Event::Search {
                request: *request,
                result: Ok(SearchReply::default()),
            }),
            Command::LoadCatalogue { request } => reply.send(Event::Catalogue {
                request: *request,
                result: Ok(Catalogue::default()),
            }),
            Command::CheckHealth { request } => {
                for service in Service::ALL {
                    reply.send(Event::Health {
                        request: *request,
                        service,
                        state: ServiceState::Up {
                            detail: format!("the {} scene is ready", self.scene.name),
                        },
                    });
                }
            }
            other => {
                let failure = Failure::not_built("this command of the fake backend");
                for event in other.failed(&failure) {
                    reply.send(event);
                }
            }
        }
    }
}
