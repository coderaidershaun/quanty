//! The second half of extraction: writes what was found to the graph, in item order and one
//! concept at a time, so that a concept that an earlier item made is found by a later one.

use std::collections::HashMap;

use graph::{GraphError, GraphStore, Mention, Relation};
use rag_core::{ConceptId, Embedder, ItemId, Llm};

use super::question::Extraction;
use super::resolve::{Resolved, Resolver, normalised};
use super::{ConceptError, ConceptSummary};

// SMELL: nothing removes the mentions of an earlier run, so after a change of the prompt an item
// keeps the mentions of the old answers beside the new ones, until its document is deleted and
// ingested again.
// SMELL: this function does two jobs for each item in one long loop: it resolves the concepts and
// collects their mentions, and then it builds the relations. Each job wants a function of its own.
/// Writes the concepts, mentions and relations of every item. The returned summary counts only
/// these writes and the questions that resolving the concepts asked. `items` is how many items
/// the run was given, which a stop reports.
///
/// # Errors
/// - [`ConceptError::Stopped`] when a question about two concepts fails in a way that would fail
///   every other question too. What was linked before it stays.
/// - Every error that [`Resolver::resolve`] returns
/// - [`ConceptError::Graph`] when the graph cannot be read or written
pub(super) async fn write<L: Llm, E: Embedder, G: GraphStore>(
    resolver: &Resolver<'_, L, E, G>,
    extractions: &[(ItemId, Extraction)],
    items: usize,
) -> Result<ConceptSummary, ConceptError> {
    let graph = &resolver.stores.graph;
    let mut summary = ConceptSummary::default();
    for (read, (item, extraction)) in extractions.iter().enumerate() {
        // The concepts of this reply are kept by normalised name, so that a relation finds them
        // without asking the graph.
        let mut names: HashMap<String, ConceptId> = HashMap::new();
        let mut mentions: Vec<Mention> = Vec::new();
        for concept in &extraction.concepts {
            let normalised_name = normalised(&concept.name);
            if normalised_name.is_empty() {
                tracing::warn!(
                    name = concept.name,
                    "a concept with no letters or digits in its name was left out"
                );
                continue;
            }
            if names.contains_key(&normalised_name) {
                continue;
            }
            let id = match resolver.resolve(*item, concept, &mut summary).await? {
                Resolved::Created(id) => {
                    summary.concepts_created += 1;
                    id
                }
                Resolved::Linked(id) => {
                    summary.concepts_linked += 1;
                    id
                }
                Resolved::Stopped(source) => {
                    // SMELL: `read` counts the items whose concepts were written. An item that
                    // was skipped is in neither count, so the message can name fewer items than
                    // the run has dealt with.
                    return Err(ConceptError::Stopped {
                        read,
                        items,
                        source,
                    });
                }
            };
            names.insert(normalised_name, id);
            // Two names of one reply can be one concept, for example when the second became an
            // alias of the first. The item mentions it once, in the first wording.
            if !mentions.iter().any(|mention| mention.concept == id) {
                mentions.push(Mention {
                    item: *item,
                    concept: id,
                    wording: concept.name.clone(),
                });
            }
        }
        graph.add_mentions(&mentions).await?;

        let mut relations: Vec<Relation> = Vec::new();
        for relation in &extraction.relations {
            let from = concept_named(graph, &names, &relation.from).await?;
            let to = concept_named(graph, &names, &relation.to).await?;
            match (from, to) {
                (Some(from), Some(to)) if from != to => {
                    let relation = Relation {
                        from,
                        to,
                        kind: relation.kind,
                        item: *item,
                    };
                    if !relations.contains(&relation) {
                        relations.push(relation);
                    }
                }
                _ => summary.relations_dropped += 1,
            }
        }
        graph.add_relations(&relations).await?;

        summary.mentions_written += mentions.len();
        summary.relations_written += relations.len();
    }
    Ok(summary)
}

/// A concept of the same reply, or else a stored one.
async fn concept_named<G: GraphStore>(
    graph: &G,
    names: &HashMap<String, ConceptId>,
    name: &str,
) -> Result<Option<ConceptId>, GraphError> {
    let normalised_name = normalised(name);
    if let Some(id) = names.get(&normalised_name) {
        return Ok(Some(*id));
    }
    let stored = graph.find_concept_by_name(&normalised_name).await?;
    Ok(stored.map(|concept| concept.id))
}
