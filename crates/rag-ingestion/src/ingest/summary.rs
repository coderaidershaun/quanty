//! What an ingest run reports, as it works and when it ends.

use std::fmt;

use ocr::PageProgress;
use rag_core::{DocId, ItemKind};

use super::concepts::ConceptSummary;
use super::items::Item;

/// One step of an ingest, told as it happens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IngestStep {
    /// A step of the conversion of the PDF's pages.
    Converting(PageProgress),
    /// The document and its items are about to be written to the graph.
    WritingGraph,
    /// All `items` are about to be embedded, in one call.
    Embedding { items: usize },
    /// The points are about to be stored.
    Storing,
    /// `done` of `total` items were read for their concepts (a skipped item counts as read).
    ReadingConcepts { done: usize, total: usize },
    /// The concepts of `done` of `total` read items were written to the graph.
    LinkingConcepts { done: usize, total: usize },
}

/// Where the steps of a run are told. It is `Send` because the app and the agent server run an
/// ingest on a task of its own.
pub(crate) type OnStep<'a> = &'a mut (dyn FnMut(IngestStep) + Send);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ItemCounts {
    pub chunks: usize,
    pub formulas: usize,
    pub figures: usize,
    pub tables: usize,
}

impl ItemCounts {
    pub fn total(&self) -> usize {
        self.chunks + self.formulas + self.figures + self.tables
    }

    pub(super) fn of(items: &[Item]) -> Self {
        let mut counts = Self::default();
        for item in items {
            match item.payload.kind {
                ItemKind::Chunk => counts.chunks += 1,
                ItemKind::Formula => counts.formulas += 1,
                ItemKind::Figure => counts.figures += 1,
                ItemKind::Table => counts.tables += 1,
            }
        }
        counts
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestSummary {
    pub doc_id: DocId,
    pub doc_title: String,
    pub collection: String,
    pub items_by_kind: ItemCounts,
    /// The count of the whole collection after the run, not only of this chapter.
    pub points_in_collection: u64,
    pub concepts: ConceptSummary,
}

impl fmt::Display for IngestSummary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let counts = &self.items_by_kind;
        writeln!(formatter, "document: {}", self.doc_title)?;
        writeln!(formatter, "document id: {}", self.doc_id)?;
        writeln!(
            formatter,
            "items: {} chunks, {} formulas, {} figures, {} tables ({} in all)",
            counts.chunks,
            counts.formulas,
            counts.figures,
            counts.tables,
            counts.total()
        )?;
        writeln!(
            formatter,
            "points now in collection {}: {}",
            self.collection, self.points_in_collection
        )?;
        let concepts = &self.concepts;
        writeln!(
            formatter,
            "concepts: {} created, {} linked to an existing concept",
            concepts.concepts_created, concepts.concepts_linked
        )?;
        writeln!(formatter, "mentions written: {}", concepts.mentions_written)?;
        writeln!(
            formatter,
            "relations written: {}, dropped: {}",
            concepts.relations_written, concepts.relations_dropped
        )?;
        writeln!(
            formatter,
            "claude calls made: {}, cache hits: {}",
            concepts.llm_calls, concepts.cache_hits
        )?;
        write!(formatter, "items skipped: {}", concepts.skipped_items.len())?;
        for skipped in &concepts.skipped_items {
            write!(
                formatter,
                "\n  {} on page {} ({}): {}",
                skipped.kind.as_str(),
                skipped.page,
                skipped.id,
                skipped.reason
            )?;
        }
        Ok(())
    }
}
