//! Reads every item for the concepts it discusses and writes the concepts, their mentions and
//! their relations to the graph. Each answer of the model is kept in a folder, so that the same
//! question is asked once, and each decision about a name is added to a log.

mod ask;
mod cache;
mod decision_log;
mod link;
mod question;
mod resolve;
mod same_concept;

use std::path::PathBuf;

use graph::{GraphError, GraphStore};
use rag_core::{
    ConceptId, Config, EmbedError, Embedder, Embedding, ItemId, ItemKind, Llm, LlmError, StoreError,
};

use cache::Cache;
use decision_log::DecisionLog;
use resolve::Resolver;

pub use question::EXTRACTION_MODEL;
pub use resolve::{ASK_SCORE, LINK_SCORE};

use super::items::Item;
use super::summary::Meter;
use crate::stores::Stores;

/// The items of one document and the vector of each one, in the same order.
pub(super) struct EmbeddedItems<'a> {
    pub items: &'a [Item],
    pub vectors: &'a [Embedding],
}

/// Reads items for the concepts they discuss, and keeps every good answer in a folder so that the
/// same question is asked once.
pub struct ConceptExtractor<L> {
    llm: L,
    cache_folder: PathBuf,
    decision_log: PathBuf,
    prompt_version: String,
}

impl<L: Llm> ConceptExtractor<L> {
    /// Keeps the answers in the cache folder of the config and adds the decisions to its decision
    /// log. Both are made when they are missing.
    pub fn new(llm: L, config: &Config) -> Self {
        ConceptExtractor {
            llm,
            cache_folder: config.concept_cache_folder.clone(),
            decision_log: config.concept_decision_log.clone(),
            prompt_version: question::PROMPT_VERSION.to_owned(),
        }
    }

    /// The version is part of the name each answer is kept under. Another version asks about
    /// every item again.
    pub fn with_prompt_version(mut self, prompt_version: impl Into<String>) -> Self {
        self.prompt_version = prompt_version.into();
        self
    }

    /// Costs nothing, so an ingest asks it before it pays for an embedding.
    ///
    /// # Errors
    /// The errors of [`Llm::check_ready`].
    pub(super) async fn check_model_ready(&self) -> Result<(), LlmError> {
        self.llm.check_ready().await
    }

    /// Asks about every item, a few at a time, and then writes what was found to the graph in
    /// item order, one concept at a time, so that a later concept sees an earlier one.
    ///
    /// # Errors
    /// - [`ConceptError::Stopped`] when a question failed in a way that would fail every other
    ///   question too. A stop while the items are asked about writes nothing to the graph. A stop
    ///   while the concepts are linked leaves what was linked before it. The answers so far are
    ///   kept either way, so the next run goes on from them.
    /// - [`ConceptError::Comparison`] when the model failed twice to compare two concepts
    /// - [`ConceptError::Cache`] and [`ConceptError::DecisionLog`] when the cache folder or the
    ///   decision log cannot be used
    /// - [`ConceptError::RelatedItems`] when the stored items cannot be searched
    /// - [`ConceptError::Embed`], [`ConceptError::ConceptStore`], [`ConceptError::Graph`] and
    ///   [`ConceptError::PointWithoutNode`] when a concept cannot be resolved
    pub(super) async fn extract<E: Embedder, G: GraphStore>(
        &self,
        embedded: &EmbeddedItems<'_>,
        embedder: &E,
        stores: &Stores<G>,
        meter: &mut Meter<'_>,
    ) -> Result<ConceptSummary, ConceptError> {
        let cache = Cache::open(&self.cache_folder)?;
        // Opened before the first question, so that a log that cannot be used stops the run before
        // anything is paid for.
        let log = DecisionLog::open(&self.decision_log)?;
        let answers = self
            .read_items(&cache, embedded, &stores.items, meter)
            .await?;
        let resolver = Resolver {
            extractor: self,
            cache: &cache,
            embedder,
            stores,
            log: &log,
        };
        let mut summary =
            link::write(&resolver, &answers.extractions, embedded.items.len(), meter).await?;
        summary.llm_calls += answers.llm_calls;
        summary.cache_hits += answers.cache_hits;
        summary.skipped_items = answers.skipped;
        Ok(summary)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConceptSummary {
    pub concepts_created: usize,
    /// An extracted concept that matched a stored one.
    pub concepts_linked: usize,
    pub mentions_written: usize,
    pub relations_written: usize,
    /// A relation that names a concept that is neither in the same reply nor stored, or that
    /// joins a concept to itself.
    pub relations_dropped: usize,
    /// Every question that was sent, second tries included, about items and about two concepts.
    pub llm_calls: usize,
    /// Questions about items and about two concepts that a kept answer settled.
    pub cache_hits: usize,
    pub skipped_items: Vec<SkippedItem>,
}

/// An item that was not read because its questions failed. It is not kept, so the next run asks
/// about it again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedItem {
    pub id: ItemId,
    pub kind: ItemKind,
    pub page: u32,
    pub reason: String,
}

#[derive(thiserror::Error, Debug)]
pub enum ConceptError {
    #[error(
        "concept extraction stopped after {read} of {items} items; the items of the document are stored and can be searched, and the answers so far are kept, so run the same command again to go on"
    )]
    Stopped {
        /// How many items were done before the stop.
        read: usize,
        items: usize,
        #[source]
        source: LlmError,
    },

    #[error(
        "claude could not say whether {new_name:?} and {stored_name:?} are the same concept ({reason}); nothing was guessed, the items are stored and what was linked so far stays, so run the same command again"
    )]
    Comparison {
        new_name: String,
        stored_name: String,
        reason: String,
    },

    #[error("could not embed the concept {name:?}")]
    Embed {
        name: String,
        #[source]
        source: EmbedError,
    },

    #[error("could not read or write the concepts collection")]
    ConceptStore(#[from] StoreError),

    #[error("could not search the stored items for material related to the item {item}")]
    RelatedItems {
        item: ItemId,
        #[source]
        source: StoreError,
    },

    #[error(
        "the concepts collection {collection} holds the concept {name:?} ({id}) that the graph does not hold; FALKORDB_GRAPH and QDRANT_CONCEPTS_COLLECTION must belong together; if the graph was cleared, remove the collection {collection} too and ingest the documents again"
    )]
    PointWithoutNode {
        collection: String,
        name: String,
        id: ConceptId,
    },

    #[error("could not use the decision log at {}", path.display())]
    DecisionLog {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not use the answer cache at {}", path.display())]
    Cache {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not write the concepts to the graph")]
    Graph(#[from] GraphError),
}
