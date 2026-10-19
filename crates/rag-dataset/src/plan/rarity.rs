//! What the graph says about each section: the concepts its items mention, the relations among
//! them that a training item stated, and how rarely the library mentions them.

use std::collections::{BTreeSet, HashMap, HashSet};

use graph::{ConceptNode, FalkorGraph, GraphError, Relation};
use rag_core::{ConceptId, ItemId};

use crate::documents::Split;
use crate::section::Section;

use super::{ConceptSeed, RelationSeed};

/// The least a section takes of the options, so that a section of very common concepts still gets
/// some texts.
const LOWEST_FACTOR: f64 = 0.5;
/// The factor of a section whose concepts are as rare as the typical concept of the library: it
/// takes exactly what the options give.
pub(super) const TYPICAL_FACTOR: f64 = 1.0;
/// The most a section takes of the options, so that a section of very rare concepts does not take
/// the whole budget.
const HIGHEST_FACTOR: f64 = 3.0;

/// What the graph says about one section.
pub(super) struct SectionRarity {
    /// The concepts the section's items mention.
    pub(super) concept_ids: Vec<ConceptId>,
    /// Relations among those concepts that an item of a training section stated, the rarest first.
    pub(super) relations: Vec<RelationSeed>,
    /// From `LOWEST_FACTOR` for common concepts to `HIGHEST_FACTOR` for rare ones. A section that
    /// is held out, or that has no concept in the graph, gets `TYPICAL_FACTOR`.
    pub(super) factor: f64,
}

impl SectionRarity {
    /// A section that nothing is known about, which is planned as one of common rarity.
    fn unknown() -> SectionRarity {
        SectionRarity {
            concept_ids: Vec::new(),
            relations: Vec::new(),
            factor: TYPICAL_FACTOR,
        }
    }
}

/// The concepts that the graph holds for one section and the relations among them, before the
/// section is compared with the rest of the library.
struct SectionConcepts {
    concepts: Vec<ConceptNode>,
    relations: Vec<RelationSeed>,
}

pub(super) fn without_graph(sections: &[Section]) -> Vec<SectionRarity> {
    sections.iter().map(|_| SectionRarity::unknown()).collect()
}

/// Reads the concepts of every section and the relations among those of the training sections,
/// then counts the items that mention each concept of a training section. Only the training
/// sections are compared with each other: a held-out section gets its concepts and nothing else.
pub(super) async fn read(
    sections: &[Section],
    graph: &FalkorGraph,
) -> Result<Vec<SectionRarity>, GraphError> {
    let found = concepts_of_each_section(sections, graph).await?;
    let training: Vec<&SectionConcepts> = sections
        .iter()
        .zip(&found)
        .filter(|(section, _)| is_training(section))
        .map(|(_, concepts)| concepts)
        .collect();
    let counts = graph.mention_counts(&distinct_concepts(&training)).await?;
    let typical = typical_mentions(&training, &counts);
    log_what_was_found(&training, typical);
    Ok(sections
        .iter()
        .zip(found)
        .map(|(section, concepts)| {
            if is_training(section) {
                rarity_of(concepts, &counts, typical)
            } else {
                heldout_rarity(concepts)
            }
        })
        .collect())
}

fn is_training(section: &Section) -> bool {
    section.document.split == Split::Train
}

async fn concepts_of_each_section(
    sections: &[Section],
    graph: &FalkorGraph,
) -> Result<Vec<SectionConcepts>, GraphError> {
    // The model trains on the texts that relations seed, so a relation may seed one only when an
    // item of a training section stated it. Any other item is held out, rejected or not exported.
    let training_items: HashSet<ItemId> = sections
        .iter()
        .filter(|section| is_training(section))
        .flat_map(|section| section.item_ids.iter().copied())
        .collect();
    let mut found = Vec::with_capacity(sections.len());
    for section in sections {
        let concepts = graph.concepts_for_items(&section.item_ids).await?;
        let relations = if is_training(section) {
            seeds_among(&concepts, &training_items, graph).await?
        } else {
            Vec::new()
        };
        found.push(SectionConcepts {
            concepts,
            relations,
        });
    }
    Ok(found)
}

async fn seeds_among(
    concepts: &[ConceptNode],
    training_items: &HashSet<ItemId>,
    graph: &FalkorGraph,
) -> Result<Vec<RelationSeed>, GraphError> {
    if concepts.len() < 2 {
        return Ok(Vec::new());
    }
    let ids: Vec<ConceptId> = concepts.iter().map(|concept| concept.id).collect();
    let relations = graph.relations_among(&ids).await?;
    Ok(relations
        .iter()
        .filter(|relation| training_items.contains(&relation.item))
        .filter_map(|relation| seed_of(relation, concepts))
        .collect())
}

/// The relation with the names and definitions of its two concepts, or none when one of them is
/// not among the concepts of the section.
fn seed_of(relation: &Relation, concepts: &[ConceptNode]) -> Option<RelationSeed> {
    let concept = |id: ConceptId| {
        let node = concepts.iter().find(|node| node.id == id)?;
        Some(ConceptSeed {
            id: node.id,
            name: node.name.clone(),
            definition: node.definition.clone(),
        })
    };
    Some(RelationSeed {
        kind: relation.kind.as_str().to_owned(),
        from: concept(relation.from)?,
        to: concept(relation.to)?,
        item: relation.item,
    })
}

/// The concepts of the training sections, each once, in a fixed order.
fn distinct_concepts(training: &[&SectionConcepts]) -> Vec<ConceptId> {
    training
        .iter()
        .flat_map(|found| found.concepts.iter().map(|concept| concept.id))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// The number of items that mention a concept. It is at least 1, because the section's own item
/// mentions the concept, and because the factor divides by it.
fn mentions_of(concept: ConceptId, counts: &HashMap<ConceptId, u64>) -> f64 {
    counts.get(&concept).copied().unwrap_or(0).max(1) as f64
}

/// The median number of items that mention a concept that a training section names, where a
/// concept named by two sections counts twice. Most concepts of a library are mentioned by a
/// single item, so a median over the distinct concepts would be 1, and then no section would ever
/// get more than the options give.
fn typical_mentions(
    training: &[&SectionConcepts],
    counts: &HashMap<ConceptId, u64>,
) -> Option<f64> {
    let mentions = training
        .iter()
        .flat_map(|found| &found.concepts)
        .map(|concept| mentions_of(concept.id, counts))
        .collect();
    median(mentions)
}

fn log_what_was_found(training: &[&SectionConcepts], typical: Option<f64>) {
    let with_concepts = training
        .iter()
        .filter(|found| !found.concepts.is_empty())
        .count();
    tracing::info!(
        "the graph names concepts for {with_concepts} of the {} training sections; the concept a section names is typically mentioned by {} items",
        training.len(),
        typical.unwrap_or(0.0)
    );
}

fn rarity_of(
    found: SectionConcepts,
    counts: &HashMap<ConceptId, u64>,
    typical: Option<f64>,
) -> SectionRarity {
    let factor = factor_of(&found.concepts, counts, typical);
    // The relation whose rarer concept is mentioned by the fewest items comes first. The sort is
    // stable, so relations that tie keep the order of the graph.
    let rarer_end = |relation: &RelationSeed| {
        mentions_of(relation.from.id, counts).min(mentions_of(relation.to.id, counts))
    };
    let mut relations = found.relations;
    relations.sort_by(|a, b| rarer_end(a).total_cmp(&rarer_end(b)));
    SectionRarity {
        concept_ids: found.concepts.iter().map(|concept| concept.id).collect(),
        relations,
        factor,
    }
}

/// The median, over the section's concepts, of how many times more items mention the typical
/// concept than mention this one.
fn factor_of(
    concepts: &[ConceptNode],
    counts: &HashMap<ConceptId, u64>,
    typical: Option<f64>,
) -> f64 {
    let Some(typical) = typical else {
        return TYPICAL_FACTOR;
    };
    let ratios = concepts
        .iter()
        .map(|concept| typical / mentions_of(concept.id, counts))
        .collect();
    median(ratios).map_or(TYPICAL_FACTOR, |ratio| {
        ratio.clamp(LOWEST_FACTOR, HIGHEST_FACTOR)
    })
}

fn heldout_rarity(found: SectionConcepts) -> SectionRarity {
    SectionRarity {
        concept_ids: found.concepts.iter().map(|concept| concept.id).collect(),
        ..SectionRarity::unknown()
    }
}

/// The middle value, or the mean of the two middle ones, or none for no value.
fn median(mut values: Vec<f64>) -> Option<f64> {
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len() % 2 == 1 {
        values.get(middle).copied()
    } else {
        let (below, above) = (values.get(middle.checked_sub(1)?)?, values.get(middle)?);
        Some((below + above) / 2.0)
    }
}
