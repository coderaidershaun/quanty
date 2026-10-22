//! The messages of the app: what a person asks for (`Intent`), what the backend is told to do
//! (`Command`), what comes back (`Event`) and what the shared state asks the app to do (`Effect`).

use export::Format;

use super::ask::{Answer, AskDraft, SearchReply};
use super::concept_graph::ConceptGraph;
use super::failure::Failure;
use super::health::{Balance, BalanceReads, Provider, ReadFailure, Service, ServiceState};
use super::ids::{ConceptId, DocId, NoticeId, RequestId};
use super::jobs::{
    DatasetProgress, DatasetReport, DatasetRequest, ExportFile, ExportRequest, IngestOutcome,
    IngestProgress, IngestRequest, Preflight,
};
use super::library::{
    Catalogue, ChapterContents, ChapterInsights, DocumentTagsEdit, DocumentView, MediaEdit,
    NewMedia,
};
use super::source::{PageConcept, PageView};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum Tab {
    #[default]
    Ask,
    Library,
    Ingest,
    Datasets,
}

impl Tab {
    pub const ALL: [Tab; 4] = [Tab::Ask, Tab::Library, Tab::Ingest, Tab::Datasets];
}

/// The panels of the Ask tab that a person can maximise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Panel {
    Source,
    ConceptGraph,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Intent {
    OpenTab(Tab),
    Maximise(Panel),
    RestorePanels,
    /// What Escape asks for: a running ask stops, and with none running the maximised panel is
    /// put back.
    StopOrRestore,
    ToggleHelp,
    FocusAskBar,
    CopyText(String),
    ShareAnswer,
    /// A panel asks to export a document. The shell asks where to save it.
    Export(ExportRequest),
    /// The shell's answer to `PickSavePath`: where the export is written.
    ExportPathPicked {
        export: ExportRequest,
        path: std::path::PathBuf,
    },
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
    /// Opens the Library tab on the page of that media. `None` is the group of documents with no
    /// media.
    OpenMedia {
        title: Option<String>,
    },
    /// Opens the Library tab on the page of that document, on the view that was last shown.
    OpenDocument(DocId),
    ShowDocumentView(DocumentView),
    RefreshCatalogue,
    SetDocumentTags(DocumentTagsEdit),
    DeleteDocument(DocId),
    SaveMedia(NewMedia),
    EditMedia(MediaEdit),
    /// The title of the media as the library has it.
    DeleteMedia(String),
    /// The Ingest panel asks for the file dialog. It never opens one itself.
    PickPdf,
    /// The shell's answer to `PickPdf`.
    PdfPicked(std::path::PathBuf),
    CheckIngest(IngestRequest),
    StartIngest,
    CancelIngest,
    ClearIngest,
    StartDatasetBuild(DatasetRequest),
    ClearDatasetBuild,
    DismissNotice(NoticeId),
    MarkNoticesRead,
    RecheckHealth,
    RequestQuit,
    ConfirmQuit,
    DismissQuit,
    /// What the footer's entries and the start of the app ask for.
    ReadBalances,
    OpenBillingPage(Provider),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Command {
    Ask {
        request: RequestId,
        ask: AskDraft,
    },
    Export {
        request: RequestId,
        file: ExportFile,
    },
    LoadPage {
        request: RequestId,
        doc: DocId,
        page: u32,
        folder: Option<std::path::PathBuf>,
    },
    /// `folder` is the document's folder from the catalogue when it is known, and else `None`.
    LoadChapterContents {
        request: RequestId,
        doc: DocId,
        folder: Option<std::path::PathBuf>,
    },
    LoadCatalogue {
        request: RequestId,
    },
    SetDocumentTags {
        request: RequestId,
        edit: DocumentTagsEdit,
    },
    DeleteDocument {
        request: RequestId,
        doc: DocId,
    },
    SaveMedia {
        request: RequestId,
        media: NewMedia,
    },
    EditMedia {
        request: RequestId,
        edit: MediaEdit,
    },
    DeleteMedia {
        request: RequestId,
        title: String,
    },
    Preflight {
        request: RequestId,
        ingest: IngestRequest,
    },
    Ingest {
        request: RequestId,
        ingest: IngestRequest,
    },
    BuildDataset {
        request: RequestId,
        build: DatasetRequest,
    },
    CheckHealth {
        request: RequestId,
    },
    /// `folder` is the document's folder from the catalogue when it is known, and else `None`.
    LoadChapterInsights {
        request: RequestId,
        doc: DocId,
        folder: Option<std::path::PathBuf>,
    },
    Cancel(RequestId),
    ReadBalances {
        request: RequestId,
    },
}

impl Command {
    /// For `Cancel` it is the request to stop.
    pub fn request(&self) -> RequestId {
        match self {
            Command::Ask { request, .. }
            | Command::Export { request, .. }
            | Command::LoadPage { request, .. }
            | Command::LoadChapterContents { request, .. }
            | Command::LoadCatalogue { request }
            | Command::SetDocumentTags { request, .. }
            | Command::DeleteDocument { request, .. }
            | Command::SaveMedia { request, .. }
            | Command::EditMedia { request, .. }
            | Command::DeleteMedia { request, .. }
            | Command::Preflight { request, .. }
            | Command::Ingest { request, .. }
            | Command::BuildDataset { request, .. }
            | Command::CheckHealth { request }
            | Command::LoadChapterInsights { request, .. }
            | Command::ReadBalances { request } => *request,
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
            Command::Export { request, file } => vec![Event::Exported {
                request: *request,
                format: file.format,
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
            // The concepts are owed even when the contents failed: they end the load.
            Command::LoadChapterContents { request, .. } => vec![
                Event::ChapterContents {
                    request: *request,
                    result: Err(failure.clone()),
                },
                Event::ChapterConcepts {
                    request: *request,
                    result: Err(failure.clone()),
                },
            ],
            Command::LoadCatalogue { request } => vec![Event::Catalogue {
                request: *request,
                result: Err(failure.clone()),
            }],
            Command::SetDocumentTags { request, edit } => vec![Event::DocumentTagsSaved {
                request: *request,
                doc: edit.doc,
                result: Err(failure.clone()),
            }],
            Command::DeleteDocument { request, doc } => vec![Event::DocumentDeleted {
                request: *request,
                doc: *doc,
                result: Err(failure.clone()),
            }],
            Command::SaveMedia { request, .. } => vec![Event::MediaSaved {
                request: *request,
                result: Err(failure.clone()),
            }],
            Command::EditMedia { request, .. } => vec![Event::MediaEdited {
                request: *request,
                result: Err(failure.clone()),
            }],
            Command::DeleteMedia { request, .. } => vec![Event::MediaDeleted {
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
            Command::BuildDataset { request, .. } => vec![Event::DatasetFinished {
                request: *request,
                result: Err(failure.clone()),
            }],
            Command::CheckHealth { request } => unknown_health(*request).collect(),
            Command::LoadChapterInsights { request, .. } => vec![Event::ChapterInsights {
                request: *request,
                result: Err(failure.clone()),
            }],
            Command::Cancel(_) => Vec::new(),
            // A read that broke is answered, so the footer does not wait for it for ever.
            Command::ReadBalances { request } => {
                let unread = Balance::Unread(ReadFailure::Internal);
                vec![Event::Balances {
                    request: *request,
                    found: BalanceReads {
                        google: unread,
                        jev: unread,
                    },
                }]
            }
        }
    }
}

/// Every health check that learnt nothing about the services gives this answer.
pub fn unknown_health(request: RequestId) -> impl Iterator<Item = Event> {
    Service::ALL.into_iter().map(move |service| Event::Health {
        request,
        service,
        state: ServiceState::Unknown,
    })
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
    /// An export ended: `Ok` holds the path that was written.
    Exported {
        request: RequestId,
        format: Format,
        result: Result<std::path::PathBuf, Failure>,
    },
    Page {
        request: RequestId,
        result: Result<PageView, Failure>,
    },
    PageConcepts {
        request: RequestId,
        result: Result<Vec<PageConcept>, Failure>,
    },
    ChapterContents {
        request: RequestId,
        result: Result<ChapterContents, Failure>,
    },
    /// The concepts of the whole document from the graph. Always sent after `ChapterContents` of
    /// the same request, also when it fails: the state takes it as the end of the load.
    ChapterConcepts {
        request: RequestId,
        result: Result<Vec<PageConcept>, Failure>,
    },
    /// `Ok(None)`: nothing is saved for the document yet.
    ChapterInsights {
        request: RequestId,
        result: Result<Option<ChapterInsights>, Failure>,
    },
    Catalogue {
        request: RequestId,
        result: Result<Catalogue, Failure>,
    },
    DocumentTagsSaved {
        request: RequestId,
        doc: DocId,
        result: Result<(), Failure>,
    },
    DocumentDeleted {
        request: RequestId,
        doc: DocId,
        result: Result<(), Failure>,
    },
    MediaSaved {
        request: RequestId,
        result: Result<(), Failure>,
    },
    MediaEdited {
        request: RequestId,
        result: Result<(), Failure>,
    },
    MediaDeleted {
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
    DatasetProgress {
        request: RequestId,
        progress: DatasetProgress,
    },
    DatasetFinished {
        request: RequestId,
        result: Result<DatasetReport, Failure>,
    },
    Health {
        request: RequestId,
        service: Service,
        state: ServiceState,
    },
    Balances {
        request: RequestId,
        found: BalanceReads,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Effect {
    Send(Command),
    CopyText(String),
    PickFile,
    /// Ask where to save an export, and answer with `Intent::ExportPathPicked`.
    PickSavePath(ExportRequest),
    CloseWindow,
    OpenUrl(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_chapter_load_answers_every_event_it_owes() {
        let failure = Failure::not_built("the contents of a chapter");
        let request = RequestId(7);
        let doc = DocId::default();

        let contents = Command::LoadChapterContents {
            request,
            doc,
            folder: None,
        };
        assert_eq!(
            contents.failed(&failure),
            [
                Event::ChapterContents {
                    request,
                    result: Err(failure.clone()),
                },
                Event::ChapterConcepts {
                    request,
                    result: Err(failure.clone()),
                },
            ]
        );

        let insights = Command::LoadChapterInsights {
            request,
            doc,
            folder: None,
        };
        assert_eq!(
            insights.failed(&failure),
            [Event::ChapterInsights {
                request,
                result: Err(failure.clone()),
            }]
        );
    }
}
