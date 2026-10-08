//! The words every part of the app agrees on: view types, messages and shortcuts. It knows
//! nothing of the window or of the stores.

mod ask;
mod concept_graph;
mod failure;
mod health;
mod ids;
mod ingest;
mod library;
mod message;
mod notice;
mod shortcut;
mod source;

pub use ask::{
    Answer, AnswerBlock, AskDraft, AskMode, Filters, ItemKind, NothingFound, Reason, ResultItem,
    RetrievalTrace, SearchReply,
};
pub use concept_graph::{ConceptGraph, EdgeKind, GraphEdge, GraphNode, NodeId, NodeKind};
pub use failure::{Failure, FailureKind, Loadable};
pub use health::{Service, ServiceState, StartupFacts};
pub use ids::{ConceptId, DocId, ItemId, NoticeId, RequestId};
pub use ingest::{
    ChapterState, IngestOutcome, IngestProgress, IngestReport, IngestRequest, IngestStage,
    PageToCheck, Preflight,
};
pub use library::{
    Catalogue, Category, ChapterLabel, Document, DocumentName, DocumentTagsEdit, ItemCounts, Media,
    MediaEdit, NewMedia, is_same_title,
};
pub use message::{Command, Effect, Event, Intent, Tab};
pub use notice::{Notice, NoticeKind};
pub use shortcut::{Chord, KeyName, PANEL_KEYS, SHORTCUTS, Shortcut, When};
pub use source::{ImageRef, PageBox, PageConcept, PagePiece, PageView, PieceKind};
