//! The words every part of the app agrees on: view types, messages and shortcuts. It knows
//! nothing of the window or of the stores.

mod ask;
mod concept_graph;
mod failure;
mod health;
mod ids;
mod jobs;
mod library;
mod message;
mod notice;
mod shortcut;
mod source;
mod usage;

pub use ask::{
    Answer, AnswerBlock, AskDepth, AskDraft, AskMode, Filters, ItemKind, NothingFound, Reason,
    ResultItem, RetrievalTrace, SearchReply,
};
pub use concept_graph::{ConceptGraph, EdgeKind, GraphEdge, GraphNode, NodeId, NodeKind};
pub use failure::{Failure, FailureKind, Loadable};
pub use health::{
    Balance, BalanceReads, JobKind, Provider, ReadFailure, RunningJob, Service, ServiceState,
    StartupFacts,
};
pub use ids::{ConceptId, DocId, ItemId, NoticeId, RequestId, UploadId};
pub use jobs::{
    ChapterState, DatasetProgress, DatasetReport, DatasetRequest, DatasetStage, Download,
    ExportFile, ExportPlace, ExportRequest, IngestOutcome, IngestProgress, IngestReport,
    IngestRequest, IngestStage, InsightsOutcome, PageToCheck, Preflight, UploadedPdf,
};
pub use library::{
    ArtifactMetadata, Catalogue, Category, ChapterContents, ChapterInsights, ChapterItem,
    ChapterLabel, Document, DocumentName, DocumentTagsEdit, DocumentView, Insight, ItemCounts,
    Media, MediaEdit, NewMedia, is_same_name,
};
pub use message::{
    AskView, Command, Connection, Effect, Event, Frame, Intent, Panel, Tab, unknown_health,
};
pub use notice::{Notice, NoticeKind};
pub use shortcut::{Chord, KeyName, SHORTCUTS, Shortcut, When};
pub use source::{ImageRef, PageBox, PageConcept, PagePiece, PageView, PieceKind};
pub use usage::{ModelTokens, Usage};
