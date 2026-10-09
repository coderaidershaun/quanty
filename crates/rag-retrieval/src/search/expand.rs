//! The second and third steps of a search: the concepts that the question leads to, and the
//! items that mention them.

use std::collections::{BTreeMap, HashMap};

use graph::{ConceptNode, GraphStore, ItemMentions};
use rag_core::{ConceptHit, ConceptId, ConceptStore, Embedding, ItemHit, ItemId};

use super::results::Reason;
use super::{MAX_EXPANSION_ITEMS, QUESTION_CONCEPTS, SearchError};

type NamedConcept = (ConceptId, String);

pub(super) struct Expansion {
    /// The seeds and the items that the graph adds, each with the reason it is a candidate.
    pub(super) candidates: BTreeMap<ItemId, Reason>,
    /// The concepts nearest to the question, nearest first.
    pub(super) question_concepts: Vec<ConceptHit>,
    /// The concepts that the seeds mention.
    pub(super) seed_concepts: Vec<ConceptNode>,
    /// The concepts one `RELATES_TO` edge away from the two lists above.
    pub(super) related_concepts: Vec<ConceptNode>,
}

/// The concepts come in this order, each once: the ones nearest to the question, then the ones
/// that the seeds mention, then the ones one `RELATES_TO` edge away from those. An item that the
/// graph adds is shown under the first concept in that order that it mentions.
///
/// # Errors
/// - [`SearchError::Concepts`] when the nearest concepts cannot be searched
/// - [`SearchError::Graph`] when the graph cannot be read
pub(super) async fn from_seeds<G: GraphStore>(
    concepts: &ConceptStore,
    graph: &G,
    vector: &Embedding,
    seeds: &[ItemHit],
) -> Result<Expansion, SearchError> {
    let seed_ids: Vec<ItemId> = seeds.iter().map(|seed| seed.id).collect();
    let mut named: Vec<NamedConcept> = Vec::new();
    let question_concepts = concepts
        .nearest(vector.clone(), QUESTION_CONCEPTS)
        .await
        .map_err(SearchError::Concepts)?;
    for concept in &question_concepts {
        keep_once(&mut named, concept.id, &concept.name);
    }
    let seed_concepts = graph.concepts_for_items(&seed_ids).await?;
    for concept in &seed_concepts {
        keep_once(&mut named, concept.id, &concept.name);
    }
    let related_concepts = graph.related_concepts(&ids_of(&named)).await?;
    for concept in &related_concepts {
        keep_once(&mut named, concept.id, &concept.name);
    }

    let mentions = graph
        .items_for_concepts(&ids_of(&named), MAX_EXPANSION_ITEMS)
        .await?;
    Ok(Expansion {
        candidates: candidates(seed_ids, mentions, &named),
        question_concepts,
        seed_concepts,
        related_concepts,
    })
}

/// The seeds, and each other item that mentions a concept, under the first concept of `named`
/// that it mentions.
fn candidates(
    seed_ids: Vec<ItemId>,
    mentions: Vec<ItemMentions>,
    named: &[NamedConcept],
) -> BTreeMap<ItemId, Reason> {
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
    candidates
}

fn keep_once(named: &mut Vec<NamedConcept>, id: ConceptId, name: &str) {
    if !named.iter().any(|(kept, _)| *kept == id) {
        named.push((id, name.to_owned()));
    }
}

fn ids_of(named: &[NamedConcept]) -> Vec<ConceptId> {
    named.iter().map(|(id, _)| *id).collect()
}
