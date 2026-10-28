//! The `concept_neighbours` and `concept_path` tools: the relations around a concept, and the
//! shortest chain of relations between two concepts, each with the item that stated it.

use std::collections::{BTreeSet, HashMap};

use graph::{ConceptNode, Direction, FalkorGraph, ItemPlace, PathDepth, Relation, RelationKind};
use rag_core::{ConceptId, Config, ItemId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::args::{ConceptRef, bounded, parse_concept_id, stored_concept};
use super::error::ConceptsError;
use super::view::{ConceptBrief, EdgeView};
use crate::args::non_blank;

const DEFAULT_LIMIT: u32 = 50;
const MOST_EDGES: u32 = 200;
const DEEPEST_NEIGHBOURS: u32 = 2;

/// What an agent gives `concept_neighbours`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct ConceptNeighboursArgs {
    /// The `concept_id` that a result gave.
    concept_id: String,
    /// Keep only these kinds of relation: `DERIVED_FROM`, `ASSUMES`, `GENERALISES`, `PART_OF` or
    /// `USED_FOR`. All when left out.
    kinds: Option<Vec<String>>,
    /// `out` for relations that leave the concept, `in` for those that reach it, or `both`.
    /// Default `both`.
    direction: Option<String>,
    /// 1 for the concepts next to it, 2 to go one step further. Default 1.
    depth: Option<u32>,
    /// How many relations at most, from 1 to 200. Default 50.
    limit: Option<u32>,
}

#[derive(Serialize, JsonSchema)]
pub(crate) struct NeighboursResult {
    concept_id: String,
    /// The nearest relations first.
    edges: Vec<EdgeView>,
    /// Every concept that a relation touches, this one included, by name.
    concepts: Vec<ConceptBrief>,
    /// True when `limit` cut relations off.
    truncated: bool,
}

/// What an agent gives `concept_path`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct ConceptPathArgs {
    /// A `concept_id` or a name.
    from: String,
    /// A `concept_id` or a name.
    to: String,
    /// The most relations in the chain, from 1 to 4. Default 4.
    max_depth: Option<u32>,
}

#[derive(Serialize, JsonSchema)]
pub(crate) struct PathResult {
    from: ConceptBrief,
    to: ConceptBrief,
    /// In the order of the chain. Empty when no chain is that short.
    edges: Vec<EdgeView>,
    /// Every concept on the chain, the two ends included, by name. Empty with no chain.
    concepts: Vec<ConceptBrief>,
}

/// The relations around the concept: those that touch it, and with `depth` 2 those that touch the
/// concepts they reached.
///
/// # Errors
/// A bad argument, a concept that is not stored, or a graph that is not ready.
pub(crate) async fn concept_neighbours(
    config: &Config,
    args: ConceptNeighboursArgs,
) -> Result<NeighboursResult, ConceptsError> {
    let start = parse_concept_id("concept_id", &args.concept_id)?;
    let kinds = parse_kinds(args.kinds)?;
    let direction = parse_direction(args.direction)?;
    let depth = bounded("depth", args.depth, 1..=DEEPEST_NEIGHBOURS, 1)?;
    let limit = bounded("limit", args.limit, 1..=MOST_EDGES, DEFAULT_LIMIT)?;
    let graph = FalkorGraph::connect(config).await?;
    let stored = stored_concept(&graph, start).await?;

    // One more than the limit is read, to know whether the limit cut anything off.
    let mut edges = graph
        .neighbours(&[start], &kinds, direction, limit + 1)
        .await?;
    if edges.len() <= limit && depth == 2 {
        let reached: Vec<ConceptId> = touched(&edges)
            .into_iter()
            .filter(|concept| *concept != start)
            .collect();
        // Edges the first step found come again from the other end, so they are read too and
        // dropped.
        let room = limit + edges.len() + 1;
        let second = graph.neighbours(&reached, &kinds, direction, room).await?;
        for relation in second {
            if !edges.contains(&relation) {
                edges.push(relation);
            }
        }
    }
    let truncated = edges.len() > limit;
    edges.truncate(limit);

    let ids = std::iter::once(start).chain(touched(&edges)).collect();
    let concepts = briefs(&graph, ids, vec![stored]).await?;
    Ok(NeighboursResult {
        concept_id: start.to_string(),
        edges: edge_views(&graph, &edges).await?,
        concepts,
        truncated,
    })
}

/// The shortest chain of relations between two concepts, followed either way.
///
/// # Errors
/// A bad argument, a concept that is not stored, two ends that are one concept, or a graph that
/// is not ready.
pub(crate) async fn concept_path(
    config: &Config,
    args: ConceptPathArgs,
) -> Result<PathResult, ConceptsError> {
    let from = ConceptRef::id_or_name("from", &args.from)?;
    let to = ConceptRef::id_or_name("to", &args.to)?;
    let given = args.max_depth.unwrap_or(PathDepth::MAX.edges());
    let max_depth = PathDepth::new(given).ok_or(ConceptsError::OutOfRange {
        argument: "max_depth",
        given,
        least: 1,
        most: PathDepth::MAX.edges(),
    })?;
    let graph = FalkorGraph::connect(config).await?;
    let from = from.find(&graph).await?;
    let to = to.find(&graph).await?;
    if from.id == to.id {
        return Err(ConceptsError::SameConcept { name: from.name });
    }
    let chain = graph.shortest_path(from.id, to.id, max_depth).await?;
    let edges = chain.unwrap_or_default();
    let (concepts, views) = if edges.is_empty() {
        (Vec::new(), Vec::new())
    } else {
        let ids = touched(&edges).into_iter().collect();
        let concepts = briefs(&graph, ids, vec![from.clone(), to.clone()]).await?;
        (concepts, edge_views(&graph, &edges).await?)
    };
    Ok(PathResult {
        from: ConceptBrief::from(&from),
        to: ConceptBrief::from(&to),
        edges: views,
        concepts,
    })
}

fn parse_kinds(kinds: Option<Vec<String>>) -> Result<Vec<RelationKind>, ConceptsError> {
    kinds
        .unwrap_or_default()
        .iter()
        .map(|kind| kind.trim().parse().map_err(ConceptsError::RelationKind))
        .collect()
}

fn parse_direction(direction: Option<String>) -> Result<Direction, ConceptsError> {
    let Some(text) = non_blank(direction) else {
        return Ok(Direction::Both);
    };
    match text.as_str() {
        "out" => Ok(Direction::Out),
        "in" => Ok(Direction::In),
        "both" => Ok(Direction::Both),
        _ => Err(ConceptsError::NotOneOf {
            argument: "direction",
            given: text,
            allowed: "`out`, `in` or `both`",
        }),
    }
}

/// The concepts that the relations touch, each once.
fn touched(relations: &[Relation]) -> BTreeSet<ConceptId> {
    relations
        .iter()
        .flat_map(|relation| [relation.from, relation.to])
        .collect()
}

/// The concepts with these ids in brief, by name. The ones in `known` are not read again, and an
/// id the graph does not hold is left out.
///
/// SMELL: this reads one concept at a time, so a result with many relations makes many reads, up
/// to about 400 for two steps and 200 relations. A graph read of many concepts by id would make
/// it one.
async fn briefs(
    graph: &FalkorGraph,
    ids: BTreeSet<ConceptId>,
    known: Vec<ConceptNode>,
) -> Result<Vec<ConceptBrief>, ConceptsError> {
    let mut briefs: Vec<ConceptBrief> = known.iter().map(ConceptBrief::from).collect();
    for id in ids {
        if known.iter().any(|concept| concept.id == id) {
            continue;
        }
        if let Some(concept) = graph.concept(id).await? {
            briefs.push(ConceptBrief::from(&concept));
        }
    }
    briefs.sort_by_key(|concept| (concept.name.to_lowercase(), concept.concept_id.clone()));
    Ok(briefs)
}

/// The relations with the document and page of the item that stated each.
async fn edge_views(
    graph: &FalkorGraph,
    relations: &[Relation],
) -> Result<Vec<EdgeView>, ConceptsError> {
    let items: Vec<ItemId> = relations.iter().map(|relation| relation.item).collect();
    let places: HashMap<ItemId, ItemPlace> = graph.item_places(&items).await?;
    Ok(relations
        .iter()
        .map(|relation| EdgeView::new(relation, &places))
        .collect())
}
