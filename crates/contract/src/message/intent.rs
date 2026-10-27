//! What a person asks for: the tabs, the views of the Ask tab, the panels and the intents.

use crate::{
    AskDraft, ConceptId, DatasetRequest, DocId, DocumentTagsEdit, DocumentView, ExportRequest,
    IngestRequest, MediaEdit, NewMedia, NoticeId, Provider,
};

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

/// The views of the Ask tab. A small window shows one of them at a time, and a medium one shows
/// the answer beside one of the others. A wide window shows them all, so it ignores the choice.
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
pub enum AskView {
    #[default]
    Answer,
    Source,
    ConceptGraph,
    RetrievalPath,
    FollowUp,
}

impl AskView {
    /// In the order of the bottom navigation.
    pub const ALL: [AskView; 5] = [
        AskView::Answer,
        AskView::Source,
        AskView::ConceptGraph,
        AskView::RetrievalPath,
        AskView::FollowUp,
    ];

    /// The views that stand beside the Answer, in the order of the side tabs.
    pub const SIDE: [AskView; 4] = [
        AskView::Source,
        AskView::ConceptGraph,
        AskView::RetrievalPath,
        AskView::FollowUp,
    ];
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
    /// Opens the Ask tab on that view.
    ShowAskView(AskView),
    /// One step back through what the person opened.
    Back,
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
