//! What a person asks an ingest to do, what is found out before it starts, how far it has got,
//! and how it ended.

use super::failure::Failure;
use super::ids::DocId;
use super::library::{ChapterLabel, ItemCounts};

/// The book names the chapter's folder. The author and the tags are not part of the ingest: they
/// are written after it, as a change to the stored document.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct IngestRequest {
    pub pdf: std::path::PathBuf,
    pub book: String,
    pub author: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ChapterState {
    New,
    PartlyConverted { pages_done: u32 },
    Converted,
    Ingested { items: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Preflight {
    pub chapter: ChapterLabel,
    /// `None`: the number of pages could not be read.
    pub pages: Option<u32>,
    /// `None`: it could not be read, and a blocker says why.
    pub state: Option<ChapterState>,
    /// Every reason a start would fail. Empty means the ingest may start.
    pub blockers: Vec<Failure>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IngestStage {
    PreparingPages,
    Converting,
    WritingGraph,
    Embedding,
    Storing,
    ReadingConcepts,
    LinkingConcepts,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct IngestProgress {
    pub stage: IngestStage,
    pub done: Option<u32>,
    pub total: Option<u32>,
    pub pages_failed: u32,
    /// Spent so far in this run.
    pub cost_usd: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct PageToCheck {
    pub page: u32,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Default)]
pub struct IngestReport {
    pub doc: DocId,
    pub title: String,
    pub pages: u32,
    pub items: ItemCounts,
    pub concepts_created: usize,
    pub concepts_linked: usize,
    pub skipped_items: usize,
    /// `None`: nothing was converted in this run.
    pub cost_usd: Option<f64>,
    pub pages_to_check: Vec<PageToCheck>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum IngestOutcome {
    /// Nothing was converted, embedded or asked, but an author or tags that were given have
    /// still been written. `pages_to_check` is `None` when the chapter's folder is gone, so they
    /// are not known.
    AlreadyIngested {
        doc: DocId,
        items: u64,
        pages_to_check: Option<Vec<PageToCheck>>,
    },
    Ingested(IngestReport),
    Cancelled,
}
