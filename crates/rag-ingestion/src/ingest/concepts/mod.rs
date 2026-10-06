//! Reads every item for the concepts it discusses and writes them to the graph: the concepts, the
//! items that mention them, and the relations between them. Each answer of the model is kept in a
//! folder, so that the same question is asked once.

mod ask;
mod cache;
mod link;
mod question;
mod resolve;

use std::path::PathBuf;

use graph::{GraphError, GraphStore};
use rag_core::{ItemId, ItemKind, Llm, LlmError};

pub use question::EXTRACTION_MODEL;

use super::items::Item;

/// Reads items for the concepts they discuss, and keeps every good answer in a folder so that the
/// same question is asked once.
pub struct ConceptExtractor<L> {
    llm: L,
    cache_folder: PathBuf,
    prompt_version: String,
}

impl<L: Llm> ConceptExtractor<L> {
    /// Keeps the answers in `cache_folder`, which is made when it is missing.
    pub fn new(llm: L, cache_folder: impl Into<PathBuf>) -> Self {
        ConceptExtractor {
            llm,
            cache_folder: cache_folder.into(),
            prompt_version: question::PROMPT_VERSION.to_owned(),
        }
    }

    /// The version is part of the name each answer is kept under. Another version asks about
    /// every item again.
    pub fn with_prompt_version(mut self, prompt_version: impl Into<String>) -> Self {
        self.prompt_version = prompt_version.into();
        self
    }

    /// Asks about every item, a few at a time, and then writes what was found to the graph in
    /// item order.
    ///
    /// # Errors
    /// - [`ConceptError::Stopped`] when the question of an item failed in a way that would fail
    ///   every other question too. Nothing is written to the graph then, and the answers so far
    ///   are kept.
    /// - [`ConceptError::Cache`] when the cache folder cannot be used
    /// - [`ConceptError::Graph`] when the graph cannot be read or written
    pub(super) async fn extract<G: GraphStore>(
        &self,
        items: &[Item],
        graph: &G,
    ) -> Result<ConceptSummary, ConceptError> {
        let answers = self.read_items(items).await?;
        let mut summary = link::write(graph, &answers.extractions).await?;
        summary.llm_calls = answers.llm_calls;
        summary.cache_hits = answers.cache_hits;
        summary.skipped_items = answers.skipped;
        Ok(summary)
    }
}

/// What one run of extraction did.
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
    /// Every question that was sent, second tries included.
    pub llm_calls: usize,
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
        "concept extraction stopped after {read} of {items} items; the items of the chapter are stored and can be searched, and the answers so far are kept, so run the same command again to go on"
    )]
    Stopped {
        read: usize,
        items: usize,
        #[source]
        source: LlmError,
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
