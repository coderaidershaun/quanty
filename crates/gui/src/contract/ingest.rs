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

/// What an ingest is doing, with the counts of the stages that count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IngestStage {
    /// The stores are being asked whether the document is ingested already.
    CheckingStored,
    /// The PDF is being opened: what is saved is read, the key of the conversion service is
    /// tried and the pages are counted.
    OpeningPdf,
    /// The pages are cut out one at a time, all before the first one is converted. `ready` of
    /// all `pages` are saved or cut out, and an earlier run saved `saved_before` of them.
    PreparingPages {
        ready: u32,
        saved_before: u32,
        pages: u32,
    },
    /// `saved` of all `pages` are saved, by this run or by an earlier one.
    Converting {
        saved: u32,
        pages: u32,
    },
    WritingGraph,
    Embedding {
        items: u32,
    },
    Storing,
    /// `done` of all `items` are read.
    ReadingConcepts {
        done: u32,
        items: u32,
    },
    /// `done` of all `items` are linked.
    LinkingConcepts {
        done: u32,
        items: u32,
    },
}

/// The share of a page's time that cutting it out takes; converting it takes the rest. It is an
/// estimate from two measurements. Cutting a page of the sample chapter took 1.3 seconds on
/// average. The paid calls of a page took 28 seconds on average over 32 saved pages, and four
/// pages are converted at a time, so a page waits about 7 seconds. 1.3 of those 8.3 seconds is
/// about 0.15.
const CUT_SHARE_OF_A_PAGE: f32 = 0.15;

impl IngestStage {
    /// How much is done, from 0 to 1: of the work on the pages for the two page stages, of the
    /// stage for the two concept stages, and `None` for a stage that does not count what it has
    /// done.
    ///
    /// The cutting and the converting of the pages share one scale, so the share does not fall or
    /// jump where one turns into the other: a page that is cut out but not yet saved counts as a
    /// small share of a page, and a page that is saved counts as a whole page.
    pub fn share_done(self) -> Option<f32> {
        let (counts_as_done, total) = match self {
            IngestStage::PreparingPages {
                ready,
                saved_before,
                pages,
            } => {
                let cut_now = ready.saturating_sub(saved_before);
                let counts_as_done = saved_before as f32 + CUT_SHARE_OF_A_PAGE * cut_now as f32;
                (counts_as_done, pages)
            }
            IngestStage::Converting { saved, pages } => {
                let cut_not_saved = pages.saturating_sub(saved);
                let counts_as_done = saved as f32 + CUT_SHARE_OF_A_PAGE * cut_not_saved as f32;
                (counts_as_done, pages)
            }
            IngestStage::ReadingConcepts { done, items }
            | IngestStage::LinkingConcepts { done, items } => (done as f32, items),
            IngestStage::CheckingStored
            | IngestStage::OpeningPdf
            | IngestStage::WritingGraph
            | IngestStage::Embedding { .. }
            | IngestStage::Storing => return None,
        };
        if total == 0 {
            return None;
        }
        Some(counts_as_done / total as f32)
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct IngestProgress {
    pub stage: IngestStage,
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
