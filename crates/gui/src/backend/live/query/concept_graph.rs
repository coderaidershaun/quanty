//! Decides which concepts and results of an ask are drawn as a graph, reads the links between
//! them from the graph store, and keeps only what is linked, so that nothing floats free.

use std::collections::{HashMap, HashSet};

use graph::{GraphError, GraphStore, Mention, Relation, RelationKind};
use rag_core::{ConceptId, ItemId};
use rag_retrieval::SearchTrace;

use super::trace::{self, SeedConcept};
use crate::contract::{
    ConceptGraph, EdgeKind, GraphEdge, GraphNode, ItemKind, NodeId, NodeKind, ResultItem,
};

/// How many of the concepts the search started from are drawn.
const MAX_SEEDS: usize = 8;
/// How many related concepts are read, before the ones with no link to a seed are dropped.
const RELATED_POOL: usize = 40;
const MAX_RELATED: usize = 6;
/// With the caps above the graph has 20 nodes at most.
const MAX_ITEM_NODES: usize = 6;

struct RelatedConcept {
    id: ConceptId,
    name: String,
}

struct ItemCandidate {
    id: ItemId,
    node: GraphNode,
}

/// What may be drawn, before the graph store says what is linked.
pub(super) struct Candidates {
    seeds: Vec<SeedConcept>,
    pool: Vec<RelatedConcept>,
    items: Vec<ItemCandidate>,
}

impl Candidates {
    /// A chunk is never drawn: chunks that mention everything would bury the concepts.
    pub(super) fn new(trace: &SearchTrace, results: &[ResultItem]) -> Candidates {
        let seeds: Vec<SeedConcept> = trace::seeds(trace).into_iter().take(MAX_SEEDS).collect();
        let pool = trace
            .related_concepts
            .iter()
            .filter(|node| seeds.iter().all(|seed| seed.id != node.id))
            .take(RELATED_POOL)
            .map(|node| RelatedConcept {
                id: node.id,
                name: node.name.clone(),
            })
            .collect();
        let items = results.iter().filter_map(item_candidate).collect();
        Candidates { seeds, pool, items }
    }
}

fn item_candidate(result: &ResultItem) -> Option<ItemCandidate> {
    let (kind, word, detail) = match result.kind {
        ItemKind::Chunk => return None,
        ItemKind::Formula => (NodeKind::Formula, "Formula", &result.name),
        ItemKind::Figure => (NodeKind::Figure, "Figure", &result.caption),
        ItemKind::Table => (NodeKind::Table, "Table", &result.caption),
    };
    // The printed label of a figure or a table holds its word, as in "Figure 13-4". A formula's
    // does not, as in "(2.4)".
    let label = match (&result.label, result.kind) {
        (Some(label), ItemKind::Formula) => format!("{word} {label}"),
        (Some(label), _) => label.clone(),
        (None, _) => format!("{word} [{}]", result.number),
    };
    Some(ItemCandidate {
        id: result.id.into(),
        node: GraphNode {
            id: NodeId::Item(result.number),
            kind,
            label,
            detail: detail.clone(),
        },
    })
}

/// # Errors
/// [`GraphError`] when the relations or the mentions cannot be read.
pub(super) async fn read<G: GraphStore>(
    graph: &G,
    mut candidates: Candidates,
) -> Result<ConceptGraph, GraphError> {
    if candidates.seeds.is_empty() {
        return Ok(ConceptGraph::default());
    }
    let concepts: Vec<ConceptId> = candidates
        .seeds
        .iter()
        .map(|seed| seed.id)
        .chain(candidates.pool.iter().map(|concept| concept.id))
        .collect();
    let items: Vec<ItemId> = candidates.items.iter().map(|item| item.id).collect();
    let (relations, mentions) = tokio::join!(
        graph.relations_among(&concepts),
        graph.mentions_between(&items, &concepts)
    );
    let (relations, mentions) = (relations?, mentions?);
    for seed in candidates
        .seeds
        .iter_mut()
        .filter(|seed| seed.definition.is_none())
    {
        match graph.concept(seed.id).await {
            Ok(node) => seed.definition = node.map(|node| node.definition),
            Err(error) => tracing::warn!(?error, "could not read the definition of a concept"),
        }
    }
    Ok(assemble(candidates, &relations, &mentions))
}

fn assemble(candidates: Candidates, relations: &[Relation], mentions: &[Mention]) -> ConceptGraph {
    let Candidates { seeds, pool, items } = candidates;
    let relations = distinct(relations);
    let seed_ids: HashSet<ConceptId> = seeds.iter().map(|seed| seed.id).collect();
    let related = related_to_draw(pool, &relations, &seed_ids);
    let drawn: HashSet<ConceptId> = seed_ids
        .iter()
        .copied()
        .chain(related.iter().map(|concept| concept.id))
        .collect();
    let items = items_to_draw(items, mentions, &drawn);
    let mut edges = relation_edges(&relations, &drawn);
    edges.extend(mention_edges(mentions, &drawn, &items));

    let mut nodes: Vec<GraphNode> = seeds
        .into_iter()
        .map(|seed| GraphNode {
            id: NodeId::Concept(seed.id.into()),
            kind: NodeKind::Concept,
            label: seed.name,
            detail: seed.definition,
        })
        .collect();
    nodes.extend(related.into_iter().map(|concept| GraphNode {
        id: NodeId::Concept(concept.id.into()),
        kind: NodeKind::Related,
        label: concept.name,
        detail: None,
    }));
    nodes.extend(items.into_iter().map(|item| item.node));
    ConceptGraph { nodes, edges }
}

fn items_to_draw(
    items: Vec<ItemCandidate>,
    mentions: &[Mention],
    drawn: &HashSet<ConceptId>,
) -> Vec<ItemCandidate> {
    let mentioning: HashSet<ItemId> = mentions
        .iter()
        .filter(|mention| drawn.contains(&mention.concept))
        .map(|mention| mention.item)
        .collect();
    items
        .into_iter()
        .filter(|item| mentioning.contains(&item.id))
        .take(MAX_ITEM_NODES)
        .collect()
}

fn relation_edges(relations: &[&Relation], drawn: &HashSet<ConceptId>) -> Vec<GraphEdge> {
    relations
        .iter()
        .filter(|relation| drawn.contains(&relation.from) && drawn.contains(&relation.to))
        .map(|relation| GraphEdge {
            from: NodeId::Concept(relation.from.into()),
            to: NodeId::Concept(relation.to.into()),
            kind: edge_kind(relation.kind),
        })
        .collect()
}

fn mention_edges(
    mentions: &[Mention],
    drawn: &HashSet<ConceptId>,
    items: &[ItemCandidate],
) -> Vec<GraphEdge> {
    let numbers: HashMap<ItemId, NodeId> =
        items.iter().map(|item| (item.id, item.node.id)).collect();
    mentions
        .iter()
        .filter_map(|mention| {
            let item = numbers.get(&mention.item)?;
            drawn.contains(&mention.concept).then(|| GraphEdge {
                from: *item,
                to: NodeId::Concept(mention.concept.into()),
                kind: EdgeKind::Mentions,
            })
        })
        .collect()
}

/// Two items can state one relation, and it is one arrow.
fn distinct(relations: &[Relation]) -> Vec<&Relation> {
    let mut seen = HashSet::new();
    relations
        .iter()
        .filter(|relation| seen.insert((relation.from, relation.to, relation.kind)))
        .collect()
}

fn related_to_draw(
    pool: Vec<RelatedConcept>,
    relations: &[&Relation],
    seed_ids: &HashSet<ConceptId>,
) -> Vec<RelatedConcept> {
    let mut linked: Vec<(usize, RelatedConcept)> = pool
        .into_iter()
        .filter_map(|concept| {
            let to_seeds = relations
                .iter()
                .filter(|relation| {
                    (relation.from == concept.id && seed_ids.contains(&relation.to))
                        || (relation.to == concept.id && seed_ids.contains(&relation.from))
                })
                .count();
            (to_seeds > 0).then_some((to_seeds, concept))
        })
        .collect();
    linked.sort_by(|(count, concept), (other_count, other)| {
        other_count
            .cmp(count)
            .then_with(|| concept.name.cmp(&other.name))
    });
    linked
        .into_iter()
        .map(|(_, concept)| concept)
        .take(MAX_RELATED)
        .collect()
}

fn edge_kind(kind: RelationKind) -> EdgeKind {
    match kind {
        RelationKind::DerivedFrom => EdgeKind::DerivedFrom,
        RelationKind::Assumes => EdgeKind::Assumes,
        RelationKind::Generalises => EdgeKind::Generalises,
        RelationKind::PartOf => EdgeKind::PartOf,
        RelationKind::UsedFor => EdgeKind::UsedFor,
    }
}
