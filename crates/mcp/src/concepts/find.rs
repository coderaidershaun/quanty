//! The `find_concepts` tool: a concept found by its exact name or alias, which costs nothing, or
//! concepts found by meaning, which embeds the question.

use graph::{ConceptNode, FalkorGraph};
use rag_core::{ConceptStore, Config, Embedder, question_usage};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::args::bounded;
use super::error::ConceptsError;
use crate::args::non_blank;
use crate::services::Services;
use crate::usage::Spent;

const DEFAULT_LIMIT: u32 = 10;
const MOST_FOUND: u32 = 25;

/// What an agent gives `find_concepts`: exactly one of `name` and `query`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct FindConceptsArgs {
    /// Exact name or alias. Free.
    name: Option<String>,
    /// By meaning. PAID.
    query: Option<String>,
    /// Most concepts.
    limit: Option<u32>,
}

#[derive(Serialize, JsonSchema)]
pub(crate) struct FindConceptsResult {
    concepts: Vec<FoundConcept>,
    /// What the embedding used. Only with `query`.
    #[serde(flatten)]
    spent: Option<Spent>,
}

#[derive(Serialize, JsonSchema)]
struct FoundConcept {
    concept_id: String,
    name: String,
    definition: String,
    aliases: Vec<String>,
    mentions: u64,
    documents: usize,
    /// How near the concept is to the `query`, from 0 to 1. Only with `query`.
    #[serde(skip_serializing_if = "Option::is_none")]
    score: Option<f32>,
}

enum Asked {
    Name(String),
    Query(String),
}

impl FindConceptsArgs {
    fn read(self) -> Result<(Asked, usize), ConceptsError> {
        let limit = bounded("limit", self.limit, 1..=MOST_FOUND, DEFAULT_LIMIT)?;
        let asked = match (non_blank(self.name), non_blank(self.query)) {
            (Some(name), None) => Asked::Name(name),
            (None, Some(query)) => Asked::Query(query),
            _ => return Err(ConceptsError::FindArguments),
        };
        Ok((asked, limit))
    }
}

/// The concepts that match the name or the query, best first.
///
/// # Errors
/// A bad argument, a store or the embedder that is not ready. The graph is connected first, so a
/// store that is down stops a `query` before anything is billed.
pub(crate) async fn find_concepts<S: Services>(
    config: &Config,
    services: &S,
    args: FindConceptsArgs,
) -> Result<FindConceptsResult, ConceptsError> {
    let (asked, limit) = args.read()?;
    let graph = FalkorGraph::connect(config).await?;
    match asked {
        Asked::Name(name) => {
            let normalised = rag_ingestion::normalised_concept_name(&name);
            let found = graph.find_concept_by_name(&normalised).await?;
            let concepts =
                describe(&graph, found.into_iter().map(|node| (node, None)).collect()).await?;
            Ok(FindConceptsResult {
                concepts,
                spent: None,
            })
        }
        Asked::Query(query) => {
            let embedder = services.embedder(config)?;
            let store = ConceptStore::connect(config).map_err(ConceptsError::ConceptStore)?;
            let vector = embedder.embed_query(&query).await?;
            let hits = store
                .nearest(vector, limit)
                .await
                .map_err(ConceptsError::ConceptStore)?;
            let mut nodes = Vec::new();
            for hit in hits {
                // A point whose concept the graph does not hold is left out.
                if let Some(node) = graph.concept(hit.id).await? {
                    nodes.push((node, Some(hit.score)));
                }
            }
            Ok(FindConceptsResult {
                concepts: describe(&graph, nodes).await?,
                spent: Some(Spent::from(&question_usage(&query))),
            })
        }
    }
}

/// SMELL: this reads the aliases and the documents of one concept at a time, so 25 concepts make
/// 50 reads. Graph reads of many concepts at once would make it two.
async fn describe(
    graph: &FalkorGraph,
    found: Vec<(ConceptNode, Option<f32>)>,
) -> Result<Vec<FoundConcept>, ConceptsError> {
    let ids: Vec<_> = found.iter().map(|(node, _)| node.id).collect();
    let counts = graph.mention_counts(&ids).await?;
    let mut concepts = Vec::with_capacity(found.len());
    for (node, score) in found {
        let aliases = graph.aliases_of(node.id).await?;
        let documents = graph.documents_for_concept(node.id).await?.len();
        concepts.push(FoundConcept {
            concept_id: node.id.to_string(),
            name: node.name,
            definition: node.definition,
            aliases,
            mentions: counts.get(&node.id).copied().unwrap_or(0),
            documents,
            score,
        });
    }
    Ok(concepts)
}
