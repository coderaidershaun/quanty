//! What each step of a search produced, for a caller that shows how the results were found.

use graph::ConceptNode;
use rag_core::{ConceptHit, ItemHit};

use super::results::SearchResults;

/// The steps are numbered as in [`Retriever::search_traced`](super::Retriever::search_traced).
/// Step 6 is in the results: the hits after the first `kept`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SearchTrace {
    /// Before step 1: how many documents carry every wanted label. Only items of these documents
    /// can be seeds or results. `None` when no label is wanted, so no document is left out. With
    /// `Some(0)` the search stops before the question is embedded, and every other field is empty
    /// or zero.
    pub documents_searched: Option<usize>,
    /// Step 1: the items nearest to the question, nearest first. With none the search stops, and
    /// every field below is empty or zero.
    pub seeds: Vec<ItemHit>,
    /// Step 2: the concepts nearest to the question, nearest first.
    pub question_concepts: Vec<ConceptHit>,
    /// Step 2: the concepts that the seeds mention, the one that the most seeds mention first.
    /// One of them can also be among `question_concepts`.
    pub seed_concepts: Vec<ConceptNode>,
    /// Step 3: the concepts one `RELATES_TO` edge away from the concepts of step 2. None of them
    /// is a concept of step 2.
    pub related_concepts: Vec<ConceptNode>,
    /// Step 3: how many items went on to be ranked: the seeds and the items that the graph added.
    /// The graph adds items of every kind and of every document.
    pub candidates: usize,
    /// Step 4: how many of the candidates the item store gave back. It is fewer than `candidates`
    /// when a candidate is of another kind than the one asked for, is of a document that does not
    /// carry the wanted labels, or is not stored.
    pub ranked: usize,
    /// Step 5: the ranked items that were passed over because their document already had
    /// [`MAX_RESULTS_PER_DOCUMENT`](super::MAX_RESULTS_PER_DOCUMENT) results, nearest first.
    pub capped: Vec<ItemHit>,
    /// Step 5: how many results were kept. They are the first hits of the results, and the hits
    /// after them are the cited items of step 6.
    pub kept: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TracedSearch {
    pub results: SearchResults,
    pub trace: SearchTrace,
}
