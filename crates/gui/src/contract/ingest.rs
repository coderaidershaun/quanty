//! What a person asks an ingest to do, what is found out before it starts, how far it has got,
//! and how it ended.

use super::failure::Failure;
use super::ids::DocId;
use super::library::{Category, DocumentName, ItemCounts};
use super::usage::Usage;

/// The media and the name of the document name its folder. The labels of the media come from the
/// stored media, which is saved before its first document. The tags are the document's own: they
/// are not part of the ingest, and are written after it, as a change to the stored document.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IngestRequest {
    pub pdf: std::path::PathBuf,
    pub media: String,
    pub category: Category,
    pub name: DocumentName,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ChapterState {
    New,
    PartlyConverted { pages_done: u32 },
    Converted,
    Ingested { items: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Preflight {
    pub name: DocumentName,
    /// `None`: the number of pages could not be read.
    pub pages: Option<u32>,
    /// `None`: it could not be read, and a blocker says why.
    pub state: Option<ChapterState>,
    /// Every reason a start would fail. Empty means the ingest may start.
    pub blockers: Vec<Failure>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IngestStage {
    /// Reading the PDF: its pages are cut out before the first one is converted.
    PreparingPages,
    Converting,
    WritingGraph,
    Embedding,
    Storing,
    ReadingConcepts,
    LinkingConcepts,
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct IngestProgress {
    pub stage: IngestStage,
    /// PreparingPages: `total` is the number of pages. Converting: pages saved of all pages.
    /// Embedding: `total` is the number of items. ReadingConcepts and LinkingConcepts: items done
    /// of all items. `None` where a stage has no count.
    pub done: Option<u32>,
    pub total: Option<u32>,
    pub pages_failed: u32,
    /// What the run used so far.
    pub spent: Usage,
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
    /// What the whole run used.
    pub usage: Usage,
    pub pages_to_check: Vec<PageToCheck>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum IngestOutcome {
    /// Nothing was converted, embedded or asked, but own tags that were given have still been
    /// written. `pages_to_check` is `None` when the chapter's folder is gone, so they are not
    /// known.
    AlreadyIngested {
        doc: DocId,
        items: u64,
        pages_to_check: Option<Vec<PageToCheck>>,
    },
    Ingested(IngestReport),
    Cancelled,
}
