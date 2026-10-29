//! How far a search reaches: how many nearest items it starts from and how widely it follows the
//! graph from there.

use super::{MAX_EXPANSION_ITEMS, MAX_RESULTS_PER_DOCUMENT, QUESTION_CONCEPTS, RESULTS_PER_QUERY};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reach {
    /// How many of the nearest items the search starts from, and the most results it keeps.
    pub nearest: usize,
    /// How many results one document gives at most.
    pub per_document: usize,
    /// How many of the concepts nearest to the question the search starts from.
    pub question_concepts: usize,
    /// How many `RELATES_TO` edges the search follows from the concepts it started from.
    pub related_hops: usize,
    /// How many items the graph may add to the nearest ones.
    pub expansion_items: usize,
}

impl Reach {
    pub const FOCUSED: Self = Self {
        nearest: 5,
        per_document: 2,
        question_concepts: 1,
        related_hops: 0,
        expansion_items: 20,
    };

    /// The default: the reach of every search that does not choose one.
    pub const BALANCED: Self = Self {
        nearest: RESULTS_PER_QUERY,
        per_document: MAX_RESULTS_PER_DOCUMENT,
        question_concepts: QUESTION_CONCEPTS,
        related_hops: 1,
        expansion_items: MAX_EXPANSION_ITEMS,
    };

    pub const BROAD: Self = Self {
        nearest: 14,
        per_document: 5,
        question_concepts: 6,
        related_hops: 2,
        expansion_items: 120,
    };
}

impl Default for Reach {
    fn default() -> Self {
        Self::BALANCED
    }
}
