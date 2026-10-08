//! The second half of extraction: writes what was found to the graph, in item order and one
//! concept at a time, so that a concept that an earlier item made is found by a later one.

use std::collections::HashMap;

use graph::{GraphError, GraphStore, Mention, Relation};
use rag_core::{ConceptId, Embedder, Llm, LlmError};

use super::ask::ItemAnswer;
use super::resolve::{Resolved, Resolver, normalised};
use super::{ConceptError, ConceptSummary};
use crate::ingest::{IngestStep, OnStep};

enum ReplyConcepts {
    Resolved {
        names: HashMap<String, ConceptId>,
        mentions: Vec<Mention>,
    },
    /// The model cannot answer any question until the person acts.
    Stopped(LlmError),
}

#[derive(Default)]
struct ReplyRelations {
    kept: Vec<Relation>,
    dropped: usize,
}

// SMELL: nothing removes the mentions of an earlier run, so after a change of the prompt an item
// keeps the mentions of the old answers beside the new ones, until its document is deleted and
// ingested again.
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
    answers: &[ItemAnswer],
    items: usize,
    on_step: OnStep<'_>,
) -> Result<ConceptSummary, ConceptError> {
    let graph = &resolver.stores.graph;
    let mut summary = ConceptSummary::default();
    for (index, answer) in answers.iter().enumerate() {
        let (names, mentions) = match resolve_concepts(resolver, answer, &mut summary).await? {
            ReplyConcepts::Resolved { names, mentions } => (names, mentions),
            ReplyConcepts::Stopped(source) => {
                return Err(ConceptError::Stopped {
                    read: answer.items_before,
                    items,
                    source,
                });
            }
        };
        graph.add_mentions(&mentions).await?;

        let relations = relations_of(graph, answer, &names).await?;
        graph.add_relations(&relations.kept).await?;

        summary.mentions_written += mentions.len();
        summary.relations_written += relations.kept.len();
        summary.relations_dropped += relations.dropped;
        on_step(IngestStep::LinkingConcepts {
            done: index + 1,
            total: answers.len(),
        });
    }
    Ok(summary)
}

/// It stops at the first concept that the model cannot be asked about.
///
/// # Errors
/// Every error that [`Resolver::resolve`] returns.
async fn resolve_concepts<L: Llm, E: Embedder, G: GraphStore>(
    resolver: &Resolver<'_, L, E, G>,
    answer: &ItemAnswer,
    summary: &mut ConceptSummary,
) -> Result<ReplyConcepts, ConceptError> {
    // The concepts of this reply are kept by normalised name, so that a relation finds them
    // without asking the graph.
    let mut names: HashMap<String, ConceptId> = HashMap::new();
    let mut mentions: Vec<Mention> = Vec::new();
    for concept in &answer.extraction.concepts {
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
        let id = match resolver.resolve(answer.item, concept, summary).await? {
            Resolved::Created(id) => {
                summary.concepts_created += 1;
                id
            }
            Resolved::Linked(id) => {
                summary.concepts_linked += 1;
                id
            }
            Resolved::Stopped(source) => return Ok(ReplyConcepts::Stopped(source)),
        };
        names.insert(normalised_name, id);
        // Two names of one reply can be one concept, for example when the second became an
        // alias of the first. The item mentions it once, in the first wording.
        if !mentions.iter().any(|mention| mention.concept == id) {
            mentions.push(Mention {
                item: answer.item,
                concept: id,
                wording: concept.name.clone(),
            });
        }
    }
    Ok(ReplyConcepts::Resolved { names, mentions })
}

async fn relations_of<G: GraphStore>(
    graph: &G,
    answer: &ItemAnswer,
    names: &HashMap<String, ConceptId>,
) -> Result<ReplyRelations, GraphError> {
    let mut relations = ReplyRelations::default();
    for relation in &answer.extraction.relations {
        let from = concept_named(graph, names, &relation.from).await?;
        let to = concept_named(graph, names, &relation.to).await?;
        match (from, to) {
            (Some(from), Some(to)) if from != to => {
                let relation = Relation {
                    from,
                    to,
                    kind: relation.kind,
                    item: answer.item,
                };
                if !relations.kept.contains(&relation) {
                    relations.kept.push(relation);
                }
            }
            _ => relations.dropped += 1,
        }
    }
    Ok(relations)
}

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
