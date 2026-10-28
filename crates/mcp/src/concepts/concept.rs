//! The `concept` tool: one concept with its definition, aliases, documents and first items, and the
//! nodes that lead from it to its items and to the concepts next to it.

use graph::FalkorGraph;
use rag_core::Config;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::args::{ConceptRef, parse_concept_id};
use super::error::ConceptsError;
use super::view::{ItemRow, concept_node, tool_node};
use crate::args::non_blank;
use crate::discovery::Discovery;

/// How many concepts next to the concept, or most mentioned in the library, the nodes lead to.
const NODE_LIMIT: usize = 20;

/// How many of its items the result shows.
const TOP_ITEMS: usize = 5;

/// What an agent gives `concept`. With neither field it gets the most mentioned concepts.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct ConceptArgs {
    /// From a result.
    concept_id: Option<String>,
    /// Exact name or alias.
    name: Option<String>,
}

impl ConceptArgs {
    /// The concept the arguments name, or `None` when they name none.
    ///
    /// # Errors
    /// - [`ConceptsError::BothConcepts`] when both fields are given
    /// - [`ConceptsError::NotAConceptId`] and [`ConceptsError::BlankConcept`] when the one that is
    ///   given is not valid
    pub(crate) fn reference(self) -> Result<Option<ConceptRef>, ConceptsError> {
        match (non_blank(self.concept_id), non_blank(self.name)) {
            (Some(_), Some(_)) => Err(ConceptsError::BothConcepts),
            (Some(id), None) => {
                let id = parse_concept_id("concept_id", &id)?;
                Ok(Some(ConceptRef::Id(id)))
            }
            (None, Some(name)) => ConceptRef::named("name", &name).map(Some),
            (None, None) => Ok(None),
        }
    }
}

#[derive(Serialize, JsonSchema)]
pub(crate) struct ConceptView {
    #[serde(flatten)]
    discovery: Discovery,
    #[serde(flatten)]
    concept: Option<ConceptDetail>,
}

#[derive(Serialize, JsonSchema)]
struct ConceptDetail {
    concept_id: String,
    name: String,
    definition: String,
    aliases: Vec<String>,
    mentions: u64,
    documents: Vec<DocumentMentions>,
    top_items: Vec<ItemRow>,
}

#[derive(Serialize, JsonSchema)]
struct DocumentMentions {
    document_id: String,
    title: String,
    mentions: u64,
}

/// The concept that `reference` names, or the nodes that lead to the most mentioned concepts when
/// there is none.
///
/// # Errors
/// A concept that is not stored, or a graph that is not ready.
pub(crate) async fn concept(
    config: &Config,
    reference: Option<ConceptRef>,
) -> Result<ConceptView, ConceptsError> {
    let graph = FalkorGraph::connect(config).await?;
    let Some(reference) = reference else {
        let top = graph.top_concepts(NODE_LIMIT).await?;
        return Ok(ConceptView {
            discovery: Discovery::of(
                top.iter()
                    .map(|(concept, _)| concept_node(concept))
                    .collect(),
            ),
            concept: None,
        });
    };
    let stored = reference.find(&graph).await?;
    let id = stored.id;
    let aliases = graph.aliases_of(id).await?;
    let mentions = graph.mention_counts(&[id]).await?;
    let documents = graph.documents_for_concept(id).await?;
    let top_items = graph.items_mentioning(id, None, None, 0, TOP_ITEMS).await?;
    let next_to = graph.related_concepts(&[id]).await?;

    let mut nodes = vec![
        tool_node("Items that mention it", "concept_items", id),
        tool_node("Relations", "concept_neighbours", id),
    ];
    nodes.extend(next_to.iter().take(NODE_LIMIT).map(concept_node));
    Ok(ConceptView {
        discovery: Discovery::of(nodes),
        concept: Some(ConceptDetail {
            concept_id: id.to_string(),
            name: stored.name,
            definition: stored.definition,
            aliases,
            mentions: mentions.get(&id).copied().unwrap_or(0),
            documents: documents
                .into_iter()
                .map(|document| DocumentMentions {
                    document_id: document.document.to_string(),
                    title: document.title,
                    mentions: document.mentions,
                })
                .collect(),
            top_items: top_items.into_iter().map(ItemRow::from).collect(),
        }),
    })
}
