//! Finds the stored items for a question: the nearest ones, and those that the graph leads to. A
//! traced search also gives what each step produced.

mod cited;
mod expand;
mod rank;
mod results;
mod trace;

use graph::{GraphError, GraphStore};
use rag_core::{
    ConceptStore, DocId, EmbedError, Embedder, ItemFilter, ItemKind, ItemStore, LabelFilter,
    StoreError, UsageTally, question_usage,
};

pub use results::{Reason, SearchHit, SearchResults};
pub(crate) use results::{page_text, write_usage};
pub use trace::{SearchTrace, TracedSearch};

// The four numbers below are starting values.

/// The search also starts from this many of the nearest items.
pub const RESULTS_PER_QUERY: usize = 8;

/// How many results one document gives at most, so that one chapter cannot fill every place.
pub const MAX_RESULTS_PER_DOCUMENT: usize = 3;

/// How many of the concepts nearest to the question the search starts from, besides the concepts
/// that the nearest items mention.
const QUESTION_CONCEPTS: usize = 3;

/// How many items the graph may add to the nearest ones.
// SMELL: past this cap the graph keeps the items that mention the most concepts, not the nearest,
// and seeds and items that the filters drop take places too. It stays because only the item store
// knows nearness and the filters, so a fix needs a query that joins the two stores.
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

pub struct Retriever<E, G> {
    pub embedder: E,
    pub items: ItemStore,
    pub concepts: ConceptStore,
    pub graph: G,
}

impl<E: Embedder, G: GraphStore> Retriever<E, G> {
    /// The items for the question, without the trace of [`Retriever::search_traced`].
    ///
    /// # Errors
    /// The errors of [`Retriever::search_traced`].
    pub async fn search(
        &self,
        question: &str,
        kind: Option<ItemKind>,
        wanted: &LabelFilter,
    ) -> Result<SearchResults, SearchError> {
        Ok(self.search_traced(question, kind, wanted).await?.results)
    }

    /// The items for the question, and what each step produced, in the order of the steps:
    /// 1. the [`RESULTS_PER_QUERY`] items nearest to the question are the seeds;
    /// 2. the search starts from the concepts nearest to the question and the seed concepts, which
    ///    are the concepts that the seeds mention;
    /// 3. the graph adds the items that mention one of the concepts of step 2, and the concepts one
    ///    `RELATES_TO` edge away from them, with the items that mention those;
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
    /// With a `wanted` filter only items of the documents whose labels fit every part of it are
    /// looked at, in every step. A cited item is of the document of the result that cites it, so
    /// that document fits the filter too. When no document fits it there are no results, and the
    /// question is not embedded.
    ///
    /// # Errors
    /// - [`SearchError::Embed`] when the question cannot be embedded
    /// - [`SearchError::Items`] and [`SearchError::Concepts`] when a collection cannot be searched
    /// - [`SearchError::Graph`] when the graph cannot be read
    pub async fn search_traced(
        &self,
        question: &str,
        kind: Option<ItemKind>,
        wanted: &LabelFilter,
    ) -> Result<TracedSearch, SearchError> {
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
                return Ok(nothing_found(Some(0), UsageTally::default()));
            }
            Some(carrying)
        };
        let documents_searched = documents.as_ref().map(Vec::len);
        let filter = ItemFilter { kind, documents };
        let vector = self.embedder.embed_query(question).await?;
        let usage = question_usage(question);
        let seeds = self
            .items
            .search(vector.clone(), &filter, RESULTS_PER_QUERY)
            .await
            .map_err(SearchError::Items)?;
        if seeds.is_empty() {
            return Ok(nothing_found(documents_searched, usage));
        }
        let expansion = expand::from_seeds(&self.concepts, &self.graph, &vector, &seeds).await?;
        let candidates = expansion.candidates.len();
        let ranking =
            rank::ranked_within_the_cap(&self.items, &vector, &filter, expansion.candidates)
                .await?;
        let mut hits = ranking.kept;
        let kept = hits.len();
        if kind.is_none() {
            cited::pull_in(&self.items, &vector, &mut hits).await?;
        }
        Ok(TracedSearch {
            results: SearchResults { hits, usage },
            trace: SearchTrace {
                documents_searched,
                seeds,
                question_concepts: expansion.question_concepts,
                seed_concepts: expansion.seed_concepts,
                related_concepts: expansion.related_concepts,
                candidates,
                ranked: ranking.ranked,
                capped: ranking.capped,
                kept,
            },
        })
    }
}

fn nothing_found(documents_searched: Option<usize>, usage: UsageTally) -> TracedSearch {
    TracedSearch {
        results: SearchResults {
            hits: Vec::new(),
            usage,
        },
        trace: SearchTrace {
            documents_searched,
            ..SearchTrace::default()
        },
    }
}
