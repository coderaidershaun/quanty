//! What an ingest run reports.

use std::fmt;

use rag_core::{DocId, ItemKind};

use super::concepts::ConceptSummary;
use super::items::Item;

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
