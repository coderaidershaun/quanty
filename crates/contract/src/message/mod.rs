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

#[derive(
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Default,
)]
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
#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub enum Panel {
    Source,
    ConceptGraph,
}

#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub enum Intent {
    OpenTab(Tab),
    Maximise(Panel),
    RestorePanels,
    /// What Escape asks for: a running ask stops, and with none running the maximised panel is
    /// put back.
    StopOrRestore,
    FocusAskBar,
    CopyText(String),
    ShareAnswer,
    /// Starts an export whose download opens when the file is ready.
    Export(ExportRequest),
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
    /// The Ingest panel asks the shell to pick a PDF and upload it. It never opens the browser's
    /// file picker itself.
    PickPdf,
    /// The shell's answer to `PickPdf`.
    PdfPicked(crate::UploadedPdf),
    CheckIngest(IngestRequest),
    StartIngest,
    CancelIngest,
    ClearIngest,
    StartDatasetBuild(DatasetRequest),
    ClearDatasetBuild,
    DismissNotice(NoticeId),
    RecheckHealth,
    /// What the footer's entries and the start of the app ask for.
    ReadBalances,
    ChooseModel {
        work: choices::WorkKind,
        choice: choices::ModelSettings,
    },
    OpenBillingPage(Provider),
}

#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub enum Command {
    SaveModelChoice {
        request: RequestId,
        work: choices::WorkKind,
        choice: choices::ModelSettings,
    },
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
            Command::SaveModelChoice { request, .. }
            | Command::Ask { request, .. }
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
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, PartialOrd)]
pub enum Event {
    ModelChoiceSaved {
        request: RequestId,
        result: Result<(), Failure>,
    },
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
    /// A successful export provides a browser download.
    Exported {
        request: RequestId,
        format: Format,
        result: Result<crate::Download, Failure>,
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

#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub enum Effect {
    Send(Command),
    CopyText(String),
    PickFile,
    /// Opens a browser address, including a completed export's download.
    OpenUrl(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_chapter_load_answers_every_event_it_owes() {
        let failure = Failure::internal("the contents of a chapter could not be read");
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

mod failed;
mod frame;
pub use failed::unknown_health;
pub use frame::{Connection, Frame};
