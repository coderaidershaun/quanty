//! The messages of the app: what a person asks for (`Intent`), what the backend is told to do
//! (`Command`), what comes back (`Event`) and what the shared state asks the app to do (`Effect`).

use export::Format;

use super::ask::{Answer, AskDraft, SearchReply};
use super::concept_graph::ConceptGraph;
use super::failure::Failure;
use super::health::{Balance, BalanceReads, ReadFailure, Service, ServiceState};
use super::ids::{DocId, RequestId};
use super::jobs::{
    DatasetProgress, DatasetReport, DatasetRequest, ExportFile, IngestOutcome, IngestProgress,
    IngestRequest, Preflight,
};
use super::library::{
    Catalogue, ChapterContents, ChapterInsights, DocumentTagsEdit, MediaEdit, NewMedia,
};
use super::source::{PageConcept, PageView};

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
    /// Asks for the ingest that the server runs for the browser, or has just finished, so a tab
    /// that was reloaded shows it again. Its answer is `Event::IngestFollowed` when there is one,
    /// and nothing but the end of the request when there is none.
    FollowIngest {
        request: RequestId,
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
            | Command::FollowIngest { request }
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
    /// The ingest that `Command::FollowIngest` found. The `IngestProgress` and `IngestFinished` of
    /// that ingest come after it under the same `request`. `progress` is `None` before the first
    /// step.
    IngestFollowed {
        request: RequestId,
        ingest: IngestRequest,
        progress: Option<IngestProgress>,
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
mod intent;
pub use failed::unknown_health;
pub use frame::{Connection, Frame};
pub use intent::{AskView, Intent, Panel, Tab};
