//! Finds the stored items for a question: the nearest ones, and those that the graph leads to.

mod cited;
mod expand;
mod rank;
mod results;

use graph::{GraphError, GraphStore};
use rag_core::{
    ConceptStore, DocId, DocumentLabels, EmbedError, Embedder, ItemFilter, ItemKind, ItemStore,
    StoreError,
};

pub(crate) use results::page_text;
pub use results::{Reason, SearchHit, SearchResults};

// The four numbers below are starting values.

/// How many results a question gives. It is also how many of the nearest items the search starts
/// from.
pub const RESULTS_PER_QUERY: usize = 8;

/// How many results one document gives at most, so that one chapter cannot fill every place.
pub const MAX_RESULTS_PER_DOCUMENT: usize = 3;

/// How many of the concepts nearest to the question the search starts from, besides the concepts
/// that the nearest items mention.
const QUESTION_CONCEPTS: usize = 3;

/// How many items the graph may add to the nearest ones.
// SMELL: when more items than this mention the concepts, the graph keeps the ones that mention the
// most of them and then the ones with the lowest ids, not the ones nearest to the question. A seed
// that mentions a concept takes one of these places, and so does an item of another kind when the
// search is for one kind, and an item of a document that the labels leave out.
const MAX_EXPANSION_ITEMS: usize = 50;

#[derive(thiserror::Error, Debug)]
pub enum SearchError {
    #[error("could not embed the question")]
    Embed(#[from] EmbedError),

    #[error("could not search the stored items")]
    Items(#[source] StoreError),

    #[error("could not search the stored concepts")]
    Concepts(#[source] StoreError),

    #[error("could not read the graph")]
    Graph(#[from] GraphError),
}

/// Finds the stored items for a question: the nearest ones, and those that the graph leads to.
pub struct Retriever<E, G> {
    pub embedder: E,
    pub items: ItemStore,
    pub concepts: ConceptStore,
    pub graph: G,
}

impl<E: Embedder, G: GraphStore> Retriever<E, G> {
    /// The items for the question, in the order of the steps:
    /// 1. the [`RESULTS_PER_QUERY`] items nearest to the question are the seeds;
    /// 2. the seed concepts are the concepts nearest to the question and the concepts that the
    ///    seeds mention;
    /// 3. the graph adds the items that mention a seed concept, and the concepts one `RELATES_TO`
    ///    edge away from the seed concepts, with the items that mention those;
    /// 4. the seeds and the added items are ranked together by one query;
    /// 5. no document gives more than [`MAX_RESULTS_PER_DOCUMENT`] of the first
    ///    [`RESULTS_PER_QUERY`] results;
    /// 6. the figures, tables and equations that a result cites by their printed label come last.
    ///
    /// A seed is never ranked below an item that the graph added, so an item that came in through
    /// the graph is shown only where the cap left a place free. With no concept in the graph the
    /// results are the seeds. With a `kind` only items of that kind are looked at, and step 6 is
    /// left out.
    ///
    /// With `wanted` labels only items of the documents that carry all of them are looked at, in
    /// every step. A cited item is of the document of the result that cites it, so it is of a
    /// document that carries them too. When no document carries them there are no results, and
    /// the question is not embedded.
    ///
    /// # Errors
    /// - [`SearchError::Embed`] when the question cannot be embedded
    /// - [`SearchError::Items`] and [`SearchError::Concepts`] when a collection cannot be searched
    /// - [`SearchError::Graph`] when the graph cannot be read
    pub async fn search(
        &self,
        question: &str,
        kind: Option<ItemKind>,
        wanted: &DocumentLabels,
    ) -> Result<SearchResults, SearchError> {
        let documents = if wanted.is_empty() {
            None
        } else {
            let carrying: Vec<DocId> = self
                .graph
                .documents()
                .await?
                .into_iter()
                .filter(|node| node.labels.carries(wanted))
                .map(|node| node.id)
                .collect();
            if carrying.is_empty() {
                return Ok(SearchResults { hits: Vec::new() });
            }
            Some(carrying)
        };
        let filter = ItemFilter { kind, documents };
        let vector = self.embedder.embed_query(question).await?;
        let seeds = self
            .items
            .search(vector.clone(), &filter, RESULTS_PER_QUERY)
            .await
            .map_err(SearchError::Items)?;
        if seeds.is_empty() {
            return Ok(SearchResults { hits: Vec::new() });
        }
        let candidates = expand::candidates(&self.concepts, &self.graph, &vector, &seeds).await?;
        let mut hits =
            rank::ranked_within_the_cap(&self.items, &vector, &filter, candidates).await?;
        if kind.is_none() {
            cited::pull_in(&self.items, &vector, &mut hits).await?;
        }
        Ok(SearchResults { hits })
    }
}
