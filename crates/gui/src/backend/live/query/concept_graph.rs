//! Decides which concepts and results of an ask are drawn as a graph, reads the links between
//! them from the graph store, and keeps only what is linked, so that nothing floats free.
// SMELL: this file is over 400 lines with its test, and the limit is 500. When it grows, split it
// between reading the links from the graph store and assembling the graph; the test belongs with
// the assembling.

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
/// How many related concepts are drawn.
const MAX_RELATED: usize = 6;
/// How many results are drawn. With the caps above the graph has 20 nodes at most.
const MAX_ITEM_NODES: usize = 6;

struct RelatedConcept {
    id: ConceptId,
    name: String,
}

/// A formula, figure or table among the results, as the node it would be.
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

/// Reads the links among the candidates and assembles the graph. With no seed there is nothing to
/// draw and the store is not asked. A seed with no definition yet is read from the store by
/// itself, and one that cannot be read is drawn without a definition.
///
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

/// The graph to draw. Every seed is a node. A related concept is a node only when it has a
/// relation to a seed, and a result only when it mentions a drawn concept. An edge is drawn only
/// when both of its ends are nodes.
pub(super) fn assemble(
    candidates: Candidates,
    relations: &[Relation],
    mentions: &[Mention],
) -> ConceptGraph {
    let Candidates { seeds, pool, items } = candidates;
    let relations = distinct(relations);
    let seed_ids: HashSet<ConceptId> = seeds.iter().map(|seed| seed.id).collect();
    let related = related_to_draw(pool, &relations, &seed_ids);
    let drawn: HashSet<ConceptId> = seed_ids
        .iter()
        .copied()
        .chain(related.iter().map(|concept| concept.id))
        .collect();
    let mentioning: HashSet<ItemId> = mentions
        .iter()
        .filter(|mention| drawn.contains(&mention.concept))
        .map(|mention| mention.item)
        .collect();
    let items: Vec<ItemCandidate> = items
        .into_iter()
        .filter(|item| mentioning.contains(&item.id))
        .take(MAX_ITEM_NODES)
        .collect();
    let numbers: HashMap<ItemId, NodeId> =
        items.iter().map(|item| (item.id, item.node.id)).collect();

    let mut graph = ConceptGraph::default();
    graph.nodes.extend(seeds.into_iter().map(|seed| GraphNode {
        id: NodeId::Concept(seed.id.into()),
        kind: NodeKind::Concept,
        label: seed.name,
        detail: seed.definition,
    }));
    graph
        .nodes
        .extend(related.into_iter().map(|concept| GraphNode {
            id: NodeId::Concept(concept.id.into()),
            kind: NodeKind::Related,
            label: concept.name,
            detail: None,
        }));
    graph.nodes.extend(items.into_iter().map(|item| item.node));
    graph.edges.extend(
        relations
            .iter()
            .filter(|relation| drawn.contains(&relation.from) && drawn.contains(&relation.to))
            .map(|relation| GraphEdge {
                from: NodeId::Concept(relation.from.into()),
                to: NodeId::Concept(relation.to.into()),
                kind: edge_kind(relation.kind),
            }),
    );
    graph.edges.extend(mentions.iter().filter_map(|mention| {
        let item = numbers.get(&mention.item)?;
        drawn.contains(&mention.concept).then(|| GraphEdge {
            from: *item,
            to: NodeId::Concept(mention.concept.into()),
            kind: EdgeKind::Mentions,
        })
    }));
    graph
}

/// Two items can state one relation, and it is one arrow.
fn distinct(relations: &[Relation]) -> Vec<&Relation> {
    let mut seen = HashSet::new();
    relations
        .iter()
        .filter(|relation| seen.insert((relation.from, relation.to, relation.kind)))
        .collect()
}

/// The related concepts that have a relation to a seed, the one with the most such relations
/// first and then by name, up to the cap.
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

#[cfg(test)]
mod tests {
    use graph::ConceptNode;
    use rag_core::ConceptHit;
    use uuid::Uuid;

    use super::*;
    use crate::contract::{self, Reason};

    fn concept(name: &str) -> ConceptNode {
        ConceptNode {
            id: ConceptId::random(),
            name: name.to_owned(),
            normalised_name: name.to_lowercase(),
            definition: format!("what {name} is"),
        }
    }

    fn nearest_to_the_question(node: &ConceptNode) -> ConceptHit {
        ConceptHit {
            id: node.id,
            score: 0.9,
            name: node.name.clone(),
            aliases: Vec::new(),
        }
    }

    /// A result that has both a name and a caption, to show which of the two its node takes.
    fn result(number: usize, kind: ItemKind, label: Option<&str>) -> ResultItem {
        ResultItem {
            number,
            id: contract::ItemId(Uuid::from_u128(number as u128)),
            kind,
            score: 0.5,
            reason: Reason::Nearest,
            doc: contract::DocId(Uuid::nil()),
            doc_title: String::new(),
            book: None,
            page: 1,
            printed_page: None,
            label: label.map(str::to_owned),
            text: String::new(),
            image: None,
            piece: None,
            caption: Some(format!("caption {number}")),
            name: Some(format!("name {number}")),
        }
    }

    /// A relation that the result with the number `stated_by` states.
    fn relation(
        from: &ConceptNode,
        to: &ConceptNode,
        kind: RelationKind,
        stated_by: u128,
    ) -> Relation {
        Relation {
            from: from.id,
            to: to.id,
            kind,
            item: contract::ItemId(Uuid::from_u128(stated_by)).into(),
        }
    }

    fn mention(result: &ResultItem, concept: &ConceptNode) -> Mention {
        Mention {
            item: result.id.into(),
            concept: concept.id,
            wording: concept.name.clone(),
        }
    }

    fn concept_node(node: &ConceptNode) -> NodeId {
        NodeId::Concept(node.id.into())
    }

    fn edge(from: NodeId, to: NodeId, kind: EdgeKind) -> GraphEdge {
        GraphEdge { from, to, kind }
    }

    #[test]
    fn the_graph_keeps_seeds_related_concepts_with_an_edge_and_results_that_mention_them_within_the_caps()
     {
        // Nine concepts nearest to the question, so one is over the cap; only the first has a
        // definition from the graph.
        let s: Vec<ConceptNode> = (1..=9).map(|n| concept(&format!("S{n}"))).collect();
        // The search gives the related concepts in no useful order: here the last name first.
        let r: Vec<ConceptNode> = (1..=9).map(|n| concept(&format!("R{n}"))).collect();
        let trace = SearchTrace {
            question_concepts: s.iter().map(nearest_to_the_question).collect(),
            seed_concepts: vec![s[0].clone()],
            related_concepts: r.iter().rev().cloned().collect(),
            ..SearchTrace::default()
        };
        let results = vec![
            result(1, ItemKind::Chunk, None),
            result(2, ItemKind::Formula, Some("(2.4)")),
            result(3, ItemKind::Figure, Some("Figure 13-4")),
            result(4, ItemKind::Table, None),
            result(5, ItemKind::Formula, None),
            result(6, ItemKind::Figure, None),
            result(7, ItemKind::Formula, Some("(7.1)")),
            result(8, ItemKind::Formula, Some("(8.1)")),
            result(9, ItemKind::Formula, Some("(9.1)")),
        ];
        let (s1, s2, s3, s4, s9) = (&s[0], &s[1], &s[2], &s[3], &s[8]);
        let (r1, r2, r3, r4, r5, r6, r7, r8) =
            (&r[0], &r[1], &r[2], &r[3], &r[4], &r[5], &r[6], &r[7]);
        let relations = vec![
            relation(s1, r1, RelationKind::DerivedFrom, 1),
            // The same relation stated by a second item is one arrow, and it counts once.
            relation(s1, r1, RelationKind::DerivedFrom, 2),
            // R6 has two relations to a seed and every other concept has one, so R6 is drawn first
            // and the others follow by name.
            relation(r6, s2, RelationKind::Assumes, 1),
            relation(s1, r6, RelationKind::Assumes, 1),
            relation(s2, r2, RelationKind::Generalises, 1),
            relation(r3, s3, RelationKind::PartOf, 1),
            relation(s4, r4, RelationKind::UsedFor, 1),
            relation(s1, r5, RelationKind::DerivedFrom, 1),
            // R7 is the seventh related concept, so it is cut with its arrow.
            relation(s1, r7, RelationKind::DerivedFrom, 1),
            // S9 is over the seed cap, so it is not drawn and neither is the arrow to it.
            relation(s1, s9, RelationKind::DerivedFrom, 1),
            // R8 has no relation to a seed, only to a related concept.
            relation(r8, r1, RelationKind::PartOf, 1),
            relation(s1, s2, RelationKind::UsedFor, 1),
        ];
        let mentions = vec![
            mention(&results[1], s1),
            mention(&results[1], r1),
            // R7 was cut, so no arrow goes to it, and result 7 mentions nothing else.
            mention(&results[1], r7),
            mention(&results[6], r7),
            mention(&results[2], s2),
            mention(&results[3], s3),
            mention(&results[4], s1),
            mention(&results[5], s1),
            mention(&results[7], s1),
            // Result 9 is the seventh result that mentions a drawn concept.
            mention(&results[8], s1),
            // Result 1 is a chunk, and a chunk is never drawn.
            mention(&results[0], s1),
        ];

        let graph = assemble(Candidates::new(&trace, &results), &relations, &mentions);

        let labels: Vec<&str> = graph.nodes.iter().map(|node| node.label.as_str()).collect();
        assert_eq!(
            labels.join(", "),
            "S1, S2, S3, S4, S5, S6, S7, S8, R6, R1, R2, R3, R4, R5, \
             Formula (2.4), Figure 13-4, Table [4], Formula [5], Figure [6], Formula (8.1)"
        );
        let kind_of = |label: &str| {
            let node = graph.nodes.iter().find(|node| node.label == label);
            node.map(|node| node.kind)
        };
        assert_eq!(kind_of("S1"), Some(NodeKind::Concept));
        assert_eq!(kind_of("R1"), Some(NodeKind::Related));
        assert_eq!(kind_of("Formula (2.4)"), Some(NodeKind::Formula));
        assert_eq!(kind_of("Figure 13-4"), Some(NodeKind::Figure));
        assert_eq!(kind_of("Table [4]"), Some(NodeKind::Table));
        assert_eq!(graph.nodes[0].detail.as_deref(), Some("what S1 is"));
        assert_eq!(graph.nodes[1].detail, None);
        assert_eq!(graph.nodes[14].detail.as_deref(), Some("name 2"));
        assert_eq!(graph.nodes[15].detail.as_deref(), Some("caption 3"));
        assert_eq!(graph.nodes[16].detail.as_deref(), Some("caption 4"));
        assert_eq!(graph.nodes[19].id, NodeId::Item(8));

        let relation_edges = [
            (s1, r1, EdgeKind::DerivedFrom),
            (r6, s2, EdgeKind::Assumes),
            (s1, r6, EdgeKind::Assumes),
            (s2, r2, EdgeKind::Generalises),
            (r3, s3, EdgeKind::PartOf),
            (s4, r4, EdgeKind::UsedFor),
            (s1, r5, EdgeKind::DerivedFrom),
            (s1, s2, EdgeKind::UsedFor),
        ]
        .map(|(from, to, kind)| edge(concept_node(from), concept_node(to), kind));
        let mention_edges = [
            (2, s1),
            (2, r1),
            (3, s2),
            (4, s3),
            (5, s1),
            (6, s1),
            (8, s1),
        ]
        .map(|(number, to)| edge(NodeId::Item(number), concept_node(to), EdgeKind::Mentions));
        assert_eq!(
            graph.edges,
            [relation_edges.as_slice(), &mention_edges].concat()
        );
    }
}
