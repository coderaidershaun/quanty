//! What an ingest run reports, as it works and when it ends.

use std::fmt;

use ocr::PageProgress;
use ocr::convert::CallTally;
use rag_core::{DocId, ItemKind, ModelUsage, Usage, UsageTally, cost_text};

use super::concepts::ConceptSummary;
use super::items::Item;

/// One step of an ingest, told as it happens.
#[derive(Debug, Clone, PartialEq)]
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

/// Where the steps of a run are told, each with what the whole run used up to and with that step.
/// It is `Send` because the app and the agent server run an ingest on a task of its own.
pub(crate) type OnStep<'a> = &'a mut (dyn FnMut(IngestStep, &UsageTally) + Send);

/// Tells each step of a run, with what the run used up to that step.
pub(crate) struct Meter<'a> {
    on_step: OnStep<'a>,
    spent: UsageTally,
}

impl<'a> Meter<'a> {
    /// `spent` is what the run used before its first step, such as the conversion of a picture.
    pub(crate) fn new(on_step: OnStep<'a>, spent: UsageTally) -> Meter<'a> {
        Meter { on_step, spent }
    }

    pub(crate) fn spend(&mut self, usage: &UsageTally) {
        self.spent.add_all(usage);
    }

    pub(crate) fn step(&mut self, step: IngestStep) {
        (self.on_step)(step, &self.spent);
    }

    pub(crate) fn into_spent(self) -> UsageTally {
        self.spent
    }
}

/// What the paid calls of a conversion used, model by model, with what `claude` said each model
/// cost.
pub fn usage_of(calls: &CallTally) -> UsageTally {
    let mut usage = UsageTally::default();
    for (model, used) in &calls.by_model {
        let tokens = Usage {
            input_tokens: used.input_tokens,
            output_tokens: used.output_tokens,
            cache_read_tokens: used.cache_read_tokens,
            cache_write_tokens: used.cache_write_tokens,
        };
        let model_usage = ModelUsage {
            tokens,
            reported_usd: Some(used.cost_usd),
            estimated: false,
        };
        usage.add(model, model_usage);
    }
    usage
}

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

#[derive(Debug, Clone, PartialEq)]
pub struct IngestSummary {
    pub doc_id: DocId,
    pub doc_title: String,
    pub collection: String,
    pub items_by_kind: ItemCounts,
    /// The count of the whole collection after the run, not only of this chapter.
    pub points_in_collection: u64,
    pub concepts: ConceptSummary,
    /// What this ingest used: the embedding of its items and concepts and the concept questions,
    /// and for a picture its conversion.
    pub usage: UsageTally,
}

impl fmt::Display for IngestSummary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_counts(formatter)?;
        write_usage(formatter, &self.usage, self.usage.cost_usd())
    }
}

/// The lines of each model that `listed` holds, then the cost of the whole run, each on a line of
/// its own after what was written before.
pub(crate) fn write_usage(
    formatter: &mut fmt::Formatter<'_>,
    listed: &UsageTally,
    run_cost_usd: Option<f64>,
) -> fmt::Result {
    if !listed.is_empty() {
        write!(formatter, "\n{listed}")?;
    }
    write!(formatter, "\ncost of this run: {}", cost_text(run_cost_usd))
}

impl IngestSummary {
    /// Every line of the summary but what the run used, with no line break after the last.
    pub(crate) fn write_counts(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
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
