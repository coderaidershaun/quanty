//! The words every part of the app agrees on: view types, messages and shortcuts. It knows
//! nothing of the window or of the stores.

// SMELL: fourteen modules are more than one folder should hold. `library`, `contents` and
// `insights` all describe what the Library tab shows, and could share a folder of their own.
mod ask;
mod concept_graph;
mod contents;
mod failure;
mod health;
mod ids;
mod insights;
mod jobs;
mod library;
mod message;
mod notice;
mod shortcut;
mod source;
mod usage;

pub use ask::{
    Answer, AnswerBlock, AskDraft, AskMode, Filters, ItemKind, NothingFound, Reason, ResultItem,
    RetrievalTrace, SearchReply,
};
pub use concept_graph::{ConceptGraph, EdgeKind, GraphEdge, GraphNode, NodeId, NodeKind};
pub use contents::{ChapterContents, ChapterItem};
pub use failure::{Failure, FailureKind, Loadable};
pub use health::{Service, ServiceState, StartupFacts};
pub use ids::{ConceptId, DocId, ItemId, NoticeId, RequestId};
pub use insights::{ChapterInsights, Insight};
pub use jobs::{
    ChapterState, DatasetProgress, DatasetReport, DatasetRequest, DatasetStage, ExportFile,
    ExportPlace, ExportRequest, IngestOutcome, IngestProgress, IngestReport, IngestRequest,
    IngestStage, PageToCheck, Preflight,
};
pub use library::{
    Catalogue, Category, ChapterLabel, Document, DocumentName, DocumentTagsEdit, DocumentView,
    ItemCounts, Media, MediaEdit, NewMedia, is_same_name,
};
pub use message::{Command, Effect, Event, Intent, Panel, Tab, unknown_health};
pub use notice::{Notice, NoticeKind};
pub use shortcut::{Chord, KeyName, PANEL_KEYS, SHORTCUTS, Shortcut, When};
pub use source::{ImageRef, PageBox, PageConcept, PagePiece, PageView, PieceKind};
pub use usage::{ModelTokens, Usage};
