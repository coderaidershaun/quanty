//! What the graph says about the concepts of one document: which concepts its items mention, and
//! which items mention each of them.

use std::collections::{HashMap, HashSet};

use graph::{ConceptNode, FalkorGraph, GraphError};
use rag_core::{ConceptId, DocId, ItemId};

pub(super) struct DocumentConcepts {
    /// The concepts the items of the document mention, the most mentioned first.
    pub(super) concepts: Vec<ConceptNode>,
    items_of_concept: HashMap<ConceptId, HashSet<ItemId>>,
    concepts_of_item: HashMap<ItemId, Vec<ConceptId>>,
}

impl DocumentConcepts {
    /// The concepts of the document and the mentions of them by `items`, which are all the items
    /// that ingestion made of the document.
    ///
    /// # Errors
    /// The graph that cannot be read.
    pub(super) async fn read(
        graph: &FalkorGraph,
        document: DocId,
        items: &[ItemId],
    ) -> Result<DocumentConcepts, GraphError> {
        let concepts = graph.concepts_in_document(document).await?;
        let ids: Vec<ConceptId> = concepts.iter().map(|concept| concept.id).collect();
        let mut items_of_concept: HashMap<ConceptId, HashSet<ItemId>> = HashMap::new();
        let mut concepts_of_item: HashMap<ItemId, Vec<ConceptId>> = HashMap::new();
        for mention in graph.mentions_between(items, &ids).await? {
            items_of_concept
                .entry(mention.concept)
                .or_default()
                .insert(mention.item);
            concepts_of_item
                .entry(mention.item)
                .or_default()
                .push(mention.concept);
        }
        Ok(DocumentConcepts {
            concepts,
            items_of_concept,
            concepts_of_item,
        })
    }

    /// How many items of the document mention the concept.
    pub(super) fn mentions_of(&self, concept: ConceptId) -> usize {
        self.items_of_concept.get(&concept).map_or(0, HashSet::len)
    }

    /// How many different concepts these items mention.
    pub(super) fn concepts_in(&self, items: &[ItemId]) -> usize {
        items
            .iter()
            .filter_map(|item| self.concepts_of_item.get(item))
            .flatten()
            .collect::<HashSet<_>>()
            .len()
    }
}
