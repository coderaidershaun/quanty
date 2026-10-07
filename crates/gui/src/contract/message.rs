//! The three messages of the app: what a person asks for (`Intent`), what the backend is told to
//! do (`Command`), and what comes back (`Event`), plus what the shared state asks the app to do
//! (`Effect`).

use super::ask::{Answer, AskDraft, SearchReply};
use super::concept_graph::ConceptGraph;
use super::failure::Failure;
use super::health::{Service, ServiceState};
use super::ids::{ConceptId, DocId, NoticeId, RequestId};
use super::ingest::{IngestOutcome, IngestProgress, IngestRequest, Preflight};
use super::library::{Catalogue, LabelEdit};
use super::source::{PageConcept, PageView};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum Tab {
    #[default]
    Ask,
    Library,
    Ingest,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Ask, Tab::Library, Tab::Ingest];
}

/// What a panel or a shortcut asks for. The shared state decides what it means.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Intent {
    OpenTab(Tab),
    ToggleHelp,
    FocusAskBar,
    CopyText(String),
    ShareAnswer,
    Ask(AskDraft),
    CancelAsk,
    SelectResult(usize),
    StepResult(i32),
    FocusConcept(Option<ConceptId>),
    OpenSource {
        doc: DocId,
        page: u32,
        piece: Option<u32>,
    },
    TurnPage(i32),
    /// Load the open page and its concepts again: "Try again" in Source.
    ReloadSource,
    RefreshCatalogue,
    SetLabels(LabelEdit),
    DeleteDocument(DocId),
    /// The Ingest panel asks for the file dialog. It never opens one itself.
    PickPdf,
    /// The shell's answer to `PickPdf`. A test pushes it with no dialog.
    PdfPicked(std::path::PathBuf),
    CheckIngest(IngestRequest),
    StartIngest,
    CancelIngest,
    ClearIngest,
    DismissNotice(NoticeId),
    MarkNoticesRead,
    RecheckHealth,
    RequestQuit,
    ConfirmQuit,
    DismissQuit,
}

/// What the backend is told to do. Every command carries the id of the request it belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Command {
    Ask {
        request: RequestId,
        ask: AskDraft,
    },
    LoadPage {
        request: RequestId,
        doc: DocId,
        page: u32,
        folder: Option<std::path::PathBuf>,
    },
    LoadCatalogue {
        request: RequestId,
    },
    SetLabels {
        request: RequestId,
        edit: LabelEdit,
    },
    DeleteDocument {
        request: RequestId,
        doc: DocId,
    },
    Preflight {
        request: RequestId,
        ingest: IngestRequest,
    },
    Ingest {
        request: RequestId,
        ingest: IngestRequest,
    },
    CheckHealth {
        request: RequestId,
    },
    Cancel(RequestId),
}

impl Command {
    /// The request this command belongs to. For `Cancel`, the request to stop.
    pub fn request(&self) -> RequestId {
        match self {
            Command::Ask { request, .. }
            | Command::LoadPage { request, .. }
            | Command::LoadCatalogue { request }
            | Command::SetLabels { request, .. }
            | Command::DeleteDocument { request, .. }
            | Command::Preflight { request, .. }
            | Command::Ingest { request, .. }
            | Command::CheckHealth { request } => *request,
            Command::Cancel(request) => *request,
        }
    }

    /// The events that answer this command with a failure: what a part that is not built, a panic or a scene
    /// sends.
    pub fn failed(&self, failure: &Failure) -> Vec<Event> {
        match self {
            Command::Ask { request, .. } => vec![Event::Search {
                request: *request,
                result: Err(failure.clone()),
            }],
            Command::LoadPage { request, .. } => vec![
                Event::Page {
                    request: *request,
                    result: Err(failure.clone()),
                },
                Event::PageConcepts {
                    request: *request,
                    result: Err(failure.clone()),
                },
            ],
            Command::LoadCatalogue { request } => vec![Event::Catalogue {
                request: *request,
                result: Err(failure.clone()),
            }],
            Command::SetLabels { request, edit } => vec![Event::LabelsSaved {
                request: *request,
                doc: edit.doc,
                result: Err(failure.clone()),
            }],
            Command::DeleteDocument { request, doc } => vec![Event::Deleted {
                request: *request,
                doc: *doc,
                result: Err(failure.clone()),
            }],
            Command::Preflight { request, .. } => vec![Event::Preflight {
                request: *request,
                result: Err(failure.clone()),
            }],
            Command::Ingest { request, .. } => vec![Event::IngestFinished {
                request: *request,
                result: Err(failure.clone()),
            }],
            Command::CheckHealth { request } => Service::ALL
                .into_iter()
                .map(|service| Event::Health {
                    request: *request,
                    service,
                    state: ServiceState::Down(failure.clone()),
                })
                .collect(),
            Command::Cancel(_) => Vec::new(),
        }
    }
}

/// What the backend sends back. Every event carries the id of the command that caused it.
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum Event {
    /// The results of an ask.
    Search {
        request: RequestId,
        result: Result<SearchReply, Failure>,
    },
    /// The concept graph of an ask.
    Graph {
        request: RequestId,
        result: Result<ConceptGraph, Failure>,
    },
    /// The answer of an ask.
    Answer {
        request: RequestId,
        result: Result<Answer, Failure>,
    },
    Page {
        request: RequestId,
        result: Result<PageView, Failure>,
    },
    PageConcepts {
        request: RequestId,
        result: Result<Vec<PageConcept>, Failure>,
    },
    Catalogue {
        request: RequestId,
        result: Result<Catalogue, Failure>,
    },
    LabelsSaved {
        request: RequestId,
        doc: DocId,
        result: Result<(), Failure>,
    },
    Deleted {
        request: RequestId,
        doc: DocId,
        result: Result<(), Failure>,
    },
    Preflight {
        request: RequestId,
        result: Result<Preflight, Failure>,
    },
    IngestProgress {
        request: RequestId,
        progress: IngestProgress,
    },
    IngestFinished {
        request: RequestId,
        result: Result<IngestOutcome, Failure>,
    },
    Health {
        request: RequestId,
        service: Service,
        state: ServiceState,
    },
}

/// What the shared state asks the app to do after it applied an intent or an event.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Effect {
    Send(Command),
    CopyText(String),
    PickFile,
    CloseWindow,
}
