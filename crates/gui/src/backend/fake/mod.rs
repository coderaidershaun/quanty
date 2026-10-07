//! A backend that answers every command from built-in data, so the whole app runs with no store
//! and no model.

mod fixtures;
mod scenes;

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{Handler, Reply};
use crate::contract::{
    AskDraft, AskMode, Catalogue, Command, DocId, Event, Failure, FailureKind, Filters, Intent,
    RequestId, Service, ServiceState,
};

use scenes::{Answer, Concepts, Graph, Library, Opening, Pages, Search};
pub use scenes::{Scene, scenes};

const QDRANT_URL: &str = "http://localhost:6334";
const FALKORDB_URL: &str = "falkor://localhost:6379";

/// How long each reply takes, so a panel is seen loading before it is seen ready.
const CATALOGUE_WAIT: Duration = Duration::from_millis(200);
const SEARCH_WAIT: Duration = Duration::from_millis(400);
const GRAPH_WAIT: Duration = Duration::from_millis(150);
const ANSWER_WAIT: Duration = Duration::from_millis(1500);
const PAGE_WAIT: Duration = Duration::from_millis(100);

/// The label that no sample document carries.
const MISSING_LABEL: &str = "no-such-tag";

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pace {
    Real,
    Instant,
}

/// The fake backend of one scene.
#[derive(Debug, Clone)]
pub struct Fake {
    scene: &'static Scene,
    samples: PathBuf,
    catalogue: Catalogue,
    pace: Pace,
}

impl Fake {
    /// The fake for the scene called `name`.
    ///
    /// # Errors
    /// [`FakeError::UnknownScene`] when no scene has that name, [`FakeError::SamplesMissing`]
    /// when the sample chapters are not at or above `home`, or above the program's own folder,
    /// or cannot be read.
    pub fn scene(name: &str, home: &Path) -> Result<Fake, FakeError> {
        let scene = scenes()
            .iter()
            .find(|scene| scene.name == name)
            .ok_or_else(|| FakeError::UnknownScene {
                name: name.to_owned(),
                known: scenes()
                    .iter()
                    .map(|scene| scene.name)
                    .collect::<Vec<_>>()
                    .join(", "),
            })?;
        let missing = || FakeError::SamplesMissing {
            home: home.to_path_buf(),
        };
        let home = std::path::absolute(home).unwrap_or_else(|_| home.to_path_buf());
        let samples = fixtures::find_samples(&home)
            .or_else(|| program_folder().and_then(|folder| fixtures::find_samples(&folder)))
            .ok_or_else(missing)?;
        let catalogue = fixtures::catalogue(&samples).map_err(|error| {
            tracing::warn!(%error, "the sample chapters could not be read");
            missing()
        })?;
        Ok(Fake {
            scene,
            samples,
            catalogue: match scene.script.library {
                Library::Samples => catalogue,
                Library::Empty | Library::Fails(_) => Catalogue::default(),
            },
            pace: Pace::Real,
        })
    }

    /// The same fake with no waits, for tests. A reply that never comes still never comes.
    pub fn instant(mut self) -> Fake {
        self.pace = Pace::Instant;
        self
    }

    /// What the scene does when the window opens.
    pub fn opening(&self) -> Vec<Intent> {
        let asks = |mode, filters| {
            Intent::Ask(AskDraft {
                question: fixtures::QUESTION.to_owned(),
                mode,
                filters,
            })
        };
        match self.scene.script.opening {
            Opening::Nothing => Vec::new(),
            Opening::Asks => vec![asks(AskMode::Answer, Filters::default())],
            Opening::AsksForResultsOnly => vec![asks(AskMode::ResultsOnly, Filters::default())],
            Opening::AsksWithALabelNoDocumentHas => vec![asks(
                AskMode::Answer,
                Filters {
                    tags: vec![MISSING_LABEL.to_owned()],
                    ..Filters::default()
                },
            )],
            Opening::AsksAndOpensASource => vec![
                asks(AskMode::Answer, Filters::default()),
                Intent::OpenSource {
                    doc: DocId(uuid::Uuid::from_u128(3)),
                    page: 5,
                    piece: None,
                },
            ],
        }
    }

    async fn wait(&self, time: Duration) {
        if self.pace == Pace::Real {
            tokio::time::sleep(time).await;
        }
    }

    /// The failure of this kind, with the address in the hint where the real one has it.
    fn failure(&self, kind: FailureKind) -> Failure {
        match kind {
            FailureKind::QdrantDown => {
                Failure::new(kind, format!("could not reach Qdrant at {QDRANT_URL}"))
                    .with_hint(format!("Start Qdrant at {QDRANT_URL}, then try again."))
            }
            FailureKind::FalkorDbDown => Failure::new(
                kind,
                format!("could not connect to FalkorDB at {FALKORDB_URL}"),
            )
            .with_hint(format!("Start FalkorDB at {FALKORDB_URL}, then try again.")),
            kind => Failure::new(
                kind,
                format!("the {} scene plays this failure", self.scene.name),
            ),
        }
    }

    /// How many documents of the library carry the labels, or `None` when no label was asked.
    fn documents_with(&self, filters: &Filters) -> Option<usize> {
        let tags: Vec<String> = filters
            .tags
            .iter()
            .map(|tag| tag.trim().to_lowercase())
            .filter(|tag| !tag.is_empty())
            .collect();
        if filters.book.is_none() && filters.author.is_none() && tags.is_empty() {
            return None;
        }
        let same = |wanted: &Option<String>, found: Option<&str>| {
            wanted.as_ref().is_none_or(|wanted| {
                found.is_some_and(|found| found.to_lowercase() == wanted.to_lowercase())
            })
        };
        let count = self
            .catalogue
            .books
            .iter()
            .flat_map(|book| book.chapters.iter().map(move |document| (book, document)))
            .filter(|(book, document)| {
                same(&filters.book, book.title.as_deref())
                    && same(&filters.author, document.author.as_deref())
                    && tags.iter().all(|tag| document.tags.contains(tag))
            })
            .count();
        Some(count)
    }

    async fn ask(&self, request: RequestId, ask: &AskDraft, reply: &Reply) {
        let script = &self.scene.script;
        let search = |result| reply.send(Event::Search { request, result });
        match script.search {
            Search::Never => return std::future::pending().await,
            Search::Fails(kind) => {
                self.wait(SEARCH_WAIT).await;
                return search(Err(self.failure(kind)));
            }
            Search::Found | Search::FoundWithoutConcepts | Search::Nothing => {}
        }
        self.wait(SEARCH_WAIT).await;
        let searched = self.documents_with(&ask.filters);
        let found = match (searched, script.search) {
            (Some(0), _) => Ok(fixtures::no_document_has_the_labels()),
            (_, Search::Nothing) => Ok(fixtures::no_result()),
            (_, Search::FoundWithoutConcepts) => fixtures::reply_without_concepts(&self.samples),
            _ => fixtures::reply(&self.samples),
        };
        let mut found = match found {
            Ok(found) => found,
            Err(error) => return search(Err(Failure::internal(error.to_string()))),
        };
        if searched.is_some() {
            found.trace.documents_searched = searched;
        }
        let has_results = !found.results.is_empty();
        search(Ok(found));
        if !has_results {
            return;
        }

        self.wait(GRAPH_WAIT).await;
        let graph = match script.graph {
            Graph::Drawn => Ok(fixtures::graph()),
            Graph::Empty => Ok(Default::default()),
            Graph::Fails(kind) => Err(self.failure(kind)),
        };
        reply.send(Event::Graph {
            request,
            result: graph,
        });
        if ask.mode == AskMode::ResultsOnly {
            return;
        }

        let answer = match script.answer {
            Answer::Never => return std::future::pending().await,
            Answer::Written => Ok(fixtures::answer()),
            Answer::NoBlock => Ok(fixtures::answer_without_blocks()),
            Answer::Fails(kind) => Err(self.failure(kind)),
        };
        self.wait(ANSWER_WAIT).await;
        reply.send(Event::Answer {
            request,
            result: answer,
        });
    }

    async fn load_page(&self, request: RequestId, doc: DocId, page: u32, reply: &Reply) {
        let script = &self.scene.script;
        self.wait(PAGE_WAIT).await;
        let view = match script.pages {
            Pages::SourceMissing => Err(self.source_missing(
                "the scene says this chapter's files are not there".to_owned(),
            )),
            Pages::Samples => fixtures::page(&self.samples, doc, page).map_err(|error| {
                match error {
                    fixtures::SampleError::NoSuchPage { page_count, .. } => {
                        Failure::new(FailureKind::SourceMissing, error.to_string()).with_hint(
                            format!(
                                "This chapter has {page_count} pages, so page {page} is not in it. Ingest the chapter again with rag-ingest pdf."
                            ),
                        )
                    }
                    other => self.source_missing(other.to_string()),
                }
            }),
        };
        reply.send(Event::Page {
            request,
            result: view,
        });
        let concepts = match script.concepts {
            Concepts::Samples => Ok(fixtures::page_concepts(doc, page)),
            Concepts::Empty => Ok(Vec::new()),
            Concepts::Fails(kind) => Err(self.failure(kind)),
        };
        reply.send(Event::PageConcepts {
            request,
            result: concepts,
        });
    }

    fn source_missing(&self, detail: String) -> Failure {
        Failure::new(FailureKind::SourceMissing, detail).with_hint(format!(
            "Quanty does not know where this chapter's files are. Put its chapter folder under {}, or ingest its PDF again with rag-ingest pdf.",
            self.samples.display()
        ))
    }

    fn check_health(&self, request: RequestId, reply: &Reply) {
        for service in Service::ALL {
            let down = self
                .scene
                .script
                .down
                .iter()
                .find(|(down, _)| *down == service);
            let state = match down {
                Some((_, kind)) => ServiceState::Down(self.failure(*kind)),
                None => ServiceState::Up {
                    detail: format!("the {} scene is ready", self.scene.name),
                },
            };
            reply.send(Event::Health {
                request,
                service,
                state,
            });
        }
    }
}

/// The folder the program was started from, where the repository's samples are when it is built
/// there.
fn program_folder() -> Option<PathBuf> {
    let program = std::env::current_exe().ok()?;
    program.parent().map(Path::to_path_buf)
}

impl Handler for Fake {
    async fn serve(&self, command: Command, reply: Reply) {
        match command {
            Command::Ask { request, ask } => self.ask(request, &ask, &reply).await,
            Command::LoadPage {
                request, doc, page, ..
            } => self.load_page(request, doc, page, &reply).await,
            Command::LoadCatalogue { request } => {
                self.wait(CATALOGUE_WAIT).await;
                let result = match self.scene.script.library {
                    Library::Fails(kind) => Err(self.failure(kind)),
                    Library::Samples | Library::Empty => Ok(self.catalogue.clone()),
                };
                reply.send(Event::Catalogue { request, result });
            }
            Command::CheckHealth { request } => self.check_health(request, &reply),
            Command::Cancel(_) => {}
            other => {
                let failure = Failure::not_built("this command of the fake backend");
                for event in other.failed(&failure) {
                    reply.send(event);
                }
            }
        }
    }
}
