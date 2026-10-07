//! The second and third steps of a search: the concepts that the question leads to, and the
//! items that mention them.

use std::collections::{BTreeMap, HashMap};

use graph::GraphStore;
use rag_core::{ConceptId, ConceptStore, Embedding, ItemHit, ItemId};

use super::results::Reason;
use super::{MAX_EXPANSION_ITEMS, QUESTION_CONCEPTS, SearchError};

/// A concept with the name it is shown under.
type NamedConcept = (ConceptId, String);

/// The seeds and the items that the graph adds, each with the reason it is a candidate.
///
/// The concepts come in this order, each once: the ones nearest to the question, then the ones
/// that the seeds mention, then the ones one `RELATES_TO` edge away from those. An item that the
/// graph adds is shown under the first concept in that order that it mentions.
///
/// # Errors
/// - [`SearchError::Concepts`] when the nearest concepts cannot be searched
/// - [`SearchError::Graph`] when the graph cannot be read
pub(super) async fn candidates<G: GraphStore>(
    concepts: &ConceptStore,
    graph: &G,
    vector: &Embedding,
    seeds: &[ItemHit],
) -> Result<BTreeMap<ItemId, Reason>, SearchError> {
    let seed_ids: Vec<ItemId> = seeds.iter().map(|seed| seed.id).collect();
    let mut named: Vec<NamedConcept> = Vec::new();
    let nearest = concepts
        .nearest(vector.clone(), QUESTION_CONCEPTS)
        .await
        .map_err(SearchError::Concepts)?;
    for concept in nearest {
        keep_once(&mut named, concept.id, concept.name);
    }
    for concept in graph.concepts_for_items(&seed_ids).await? {
        keep_once(&mut named, concept.id, concept.name);
    }
    for concept in graph.related_concepts(&ids_of(&named)).await? {
        keep_once(&mut named, concept.id, concept.name);
    }

    let mentions = graph
        .items_for_concepts(&ids_of(&named), MAX_EXPANSION_ITEMS)
        .await?;
    let order: HashMap<ConceptId, usize> = named
        .iter()
        .enumerate()
        .map(|(place, (id, _))| (*id, place))
        .collect();
    let mut candidates: BTreeMap<ItemId, Reason> = seed_ids
        .into_iter()
        .map(|id| (id, Reason::Nearest))
        .collect();
    for mention in mentions {
        let first = mention
            .concepts
            .iter()
            .filter_map(|concept| order.get(concept))
            .min();
        if let (Some(place), false) = (first, candidates.contains_key(&mention.item)) {
            candidates.insert(mention.item, Reason::Concept(named[*place].1.clone()));
        }
    }
    Ok(candidates)
}

fn keep_once(named: &mut Vec<NamedConcept>, id: ConceptId, name: String) {
    if !named.iter().any(|(kept, _)| *kept == id) {
        named.push((id, name));
    }
}

fn ids_of(named: &[NamedConcept]) -> Vec<ConceptId> {
    named.iter().map(|(id, _)| *id).collect()
}
