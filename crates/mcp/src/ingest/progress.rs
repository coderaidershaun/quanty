//! Turns the steps of one ingest into the stages that the browser's progress bar shows, so that an
//! agent reads the same stages, counts and share as a person who watches the browser.

use contract::IngestStage;
use ocr::PageProgress;
use rag_ingestion::IngestStep;

use crate::jobs::Stage;

/// The page counts of one run, kept from one step to the next.
// SMELL: the browser's server counts the pages of an ingest with its own copy of these rules, so a
// change to one copy must be made to the other. Both copies belong in one place that both servers
// use.
#[derive(Default)]
pub(super) struct PageCounts {
    saved: u32,
    /// Pages saved before this run, or cut out now.
    ready: u32,
    saved_before: u32,
    pages: u32,
}

impl PageCounts {
    /// The stage that `step` turns the ingest into. `None` for a failed page: the run ends with an
    /// error that names the page, so the stage stays as it was.
    pub(super) fn stage_after(&mut self, step: &IngestStep) -> Option<IngestStage> {
        Some(match step {
            IngestStep::CheckingStored => IngestStage::CheckingStored,
            IngestStep::OpeningPdf => IngestStage::OpeningPdf,
            IngestStep::Converting(PageProgress::Pages { total, done_before }) => {
                self.pages = *total;
                self.saved = *done_before;
                self.ready = *done_before;
                self.saved_before = *done_before;
                self.preparing_or_converting()
            }
            IngestStep::Converting(PageProgress::PageCut { .. }) => {
                self.ready += 1;
                self.preparing_or_converting()
            }
            IngestStep::Converting(PageProgress::PageDone { .. }) => {
                self.saved += 1;
                self.converting()
            }
            IngestStep::Converting(PageProgress::PageFailed { .. }) => return None,
            IngestStep::WritingGraph => IngestStage::WritingGraph,
            IngestStep::Embedding { items } => IngestStage::Embedding {
                items: *items as u32,
            },
            IngestStep::Storing => IngestStage::Storing,
            IngestStep::WritingArtifacts { done, total } => IngestStage::WritingArtifacts {
                done: *done as u32,
                items: *total as u32,
            },
            IngestStep::WritingInsights => IngestStage::WritingInsights,
            IngestStep::ReadingConcepts { done, total } => IngestStage::ReadingConcepts {
                done: *done as u32,
                items: *total as u32,
            },
            IngestStep::LinkingConcepts { done, total } => IngestStage::LinkingConcepts {
                done: *done as u32,
                items: *total as u32,
            },
        })
    }

    /// The models start the moment the last page is cut, so the stage turns to converting then,
    /// not when the first page is saved.
    fn preparing_or_converting(&self) -> IngestStage {
        if self.ready < self.pages {
            IngestStage::PreparingPages {
                ready: self.ready,
                saved_before: self.saved_before,
                pages: self.pages,
            }
        } else {
            self.converting()
        }
    }

    fn converting(&self) -> IngestStage {
        IngestStage::Converting {
            saved: self.saved,
            pages: self.pages,
        }
    }
}

/// The stage as an agent reads it: the browser's name, a few words, the counts and the share.
pub(crate) fn stage_view(stage: IngestStage) -> Stage {
    let (name, words, done, total) = match stage {
        IngestStage::CheckingStored => (
            "checking_stored",
            "checking what is already stored".to_owned(),
            None,
            None,
        ),
        IngestStage::OpeningPdf => ("opening_pdf", "opening the PDF".to_owned(), None, None),
        IngestStage::PreparingPages { ready, pages, .. } => (
            "preparing_pages",
            format!("preparing the pages: {ready} of {pages} ready"),
            Some(ready),
            Some(pages),
        ),
        IngestStage::Converting { saved, pages } => (
            "converting",
            format!("converting the pages: {saved} of {pages} done"),
            Some(saved),
            Some(pages),
        ),
        IngestStage::WritingGraph => ("writing_graph", "writing the graph".to_owned(), None, None),
        IngestStage::Embedding { items } => (
            "embedding",
            format!("embedding {items} items"),
            None,
            Some(items),
        ),
        IngestStage::Storing => ("storing", "storing the items".to_owned(), None, None),
        IngestStage::WritingInsights => (
            "writing_insights",
            "writing the insights, takeaways and cheat sheet".to_owned(),
            None,
            None,
        ),
        IngestStage::WritingArtifacts { done, items } => (
            "writing_artifacts",
            format!("writing artifact descriptions: {done} of {items}"),
            Some(done),
            Some(items),
        ),
        IngestStage::ReadingConcepts { done, items } => (
            "reading_concepts",
            format!("reading the concepts: {done} of {items}"),
            Some(done),
            Some(items),
        ),
        IngestStage::LinkingConcepts { done, items } => (
            "linking_concepts",
            format!("linking the concepts: {done} of {items}"),
            Some(done),
            Some(items),
        ),
    };
    Stage::new(name, words)
        .with_counts(done.map(u64::from), total.map(u64::from))
        .with_share(stage.share_done())
}
