//! The second half of extraction: writes what was found to the graph, in item order and one
//! concept at a time, so that a concept that an earlier item made is found by a later one.

use std::collections::HashMap;

use graph::{GraphError, GraphStore, Mention, Relation};
use rag_core::{ConceptId, ItemId};

use super::ConceptSummary;
use super::question::Extraction;
use super::resolve::{Resolved, normalised, resolve};

// SMELL: nothing removes the mentions of an earlier run, so after a change of the prompt an item
// keeps the mentions of the old answers beside the new ones, until its document is deleted and
// ingested again.
/// Writes the concepts, mentions and relations of every item. The counts of the returned summary
/// are those of the graph writes only.
///
/// # Errors
/// [`GraphError`] when the graph cannot be read or written.
pub(super) async fn write<G: GraphStore>(
    graph: &G,
    extractions: &[(ItemId, Extraction)],
) -> Result<ConceptSummary, GraphError> {
    let mut summary = ConceptSummary::default();
    for (item, extraction) in extractions {
        // The concepts of this reply are kept by normalised name, so that a relation finds them
        // without asking the graph.
        let mut names: HashMap<String, ConceptId> = HashMap::new();
        let mut mentions = Vec::new();
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
            let id = match resolve(graph, concept).await? {
                Resolved::Created(id) => {
                    summary.concepts_created += 1;
                    id
                }
                Resolved::Linked(id) => {
                    summary.concepts_linked += 1;
                    id
                }
            };
            names.insert(normalised_name, id);
            mentions.push(Mention {
                item: *item,
                concept: id,
                wording: concept.name.clone(),
            });
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
    graph.find_concept_by_name(&normalised_name).await
}
