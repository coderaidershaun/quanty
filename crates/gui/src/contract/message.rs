//! The messages of the app: what a person asks for (`Intent`), what the backend is told to do
//! (`Command`), what comes back (`Event`) and what the shared state asks the app to do (`Effect`).

use super::ask::{Answer, AskDraft, SearchReply};
use super::concept_graph::ConceptGraph;
use super::failure::Failure;
use super::health::{Service, ServiceState};
use super::ids::{ConceptId, DocId, NoticeId, RequestId};
use super::ingest::{IngestOutcome, IngestProgress, IngestRequest, Preflight};
use super::library::{Catalogue, LabelEdit, NewBook};
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
    ReloadSource,
    RefreshCatalogue,
    SetLabels(LabelEdit),
    DeleteDocument(DocId),
    SaveBook(NewBook),
    /// The Ingest panel asks for the file dialog. It never opens one itself.
    PickPdf,
    /// The shell's answer to `PickPdf`.
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
    SaveBook {
        request: RequestId,
        book: NewBook,
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
    /// For `Cancel` it is the request to stop.
    pub fn request(&self) -> RequestId {
        match self {
            Command::Ask { request, .. }
            | Command::LoadPage { request, .. }
            | Command::LoadCatalogue { request }
            | Command::SetLabels { request, .. }
            | Command::DeleteDocument { request, .. }
            | Command::SaveBook { request, .. }
            | Command::Preflight { request, .. }
            | Command::Ingest { request, .. }
            | Command::CheckHealth { request } => *request,
            Command::Cancel(request) => *request,
        }
    }

    /// A health check is answered with `Unknown` for every service and not with `Down`: a check
    /// that failed says nothing about the services, and `Down` would make the next good search
    /// send the check again, for ever.
    pub fn failed(&self, failure: &Failure) -> Vec<Event> {
        match self {
            // An ask is answered in three parts, and any of them may still be waited for.
            Command::Ask { request, .. } => vec![
                Event::Search {
                    request: *request,
                    result: Err(failure.clone()),
                },
                Event::Graph {
                    request: *request,
                    result: Err(failure.clone()),
                },
                Event::Answer {
                    request: *request,
                    result: Err(failure.clone()),
                },
            ],
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
            Command::SaveBook { request, .. } => vec![Event::BookSaved {
                request: *request,
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
                    state: ServiceState::Unknown,
                })
                .collect(),
            Command::Cancel(_) => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum Event {
    Search {
        request: RequestId,
        result: Result<SearchReply, Failure>,
    },
    Graph {
        request: RequestId,
        result: Result<ConceptGraph, Failure>,
    },
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
    BookSaved {
        request: RequestId,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Effect {
    Send(Command),
    CopyText(String),
    PickFile,
    CloseWindow,
}
