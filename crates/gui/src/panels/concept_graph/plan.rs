//! Who is drawn and in which row: the centre, the order of the others, the three rows and the
//! links. It holds no number of the screen.

// SMELL: with its tests this file is over 400 lines. `Table` has one caller and is the part to
// move out, but the folder already holds twelve files, which is the most a folder may hold. Some
// of those files must be grouped before `Table` can have a file of its own.

use std::cmp::Reverse;
use std::collections::{BTreeMap, VecDeque};

use crate::contract::{ConceptGraph, ConceptId, EdgeKind, NodeId, NodeKind};

/// The most nodes one picture holds. The least important are left out and counted.
pub(super) const NODE_CAP: usize = 36;

pub(super) const TOP: usize = 0;
pub(super) const MIDDLE: usize = 1;
pub(super) const BOTTOM: usize = 2;

/// All the relations between two nodes, drawn as one line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Link {
    /// Places in `Plan::nodes`; `a < b`.
    pub a: usize,
    pub b: usize,
    /// Each kind once, in the order of `EdgeKind`.
    pub kinds: Vec<EdgeKind>,
    pub points_at_a: bool,
    pub points_at_b: bool,
}

impl Link {
    pub(super) fn touches(&self, place: usize) -> bool {
        self.a == place || self.b == place
    }

    /// The end that is not `place`.
    pub(super) fn other(&self, place: usize) -> usize {
        if self.a == place { self.b } else { self.a }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Plan {
    /// Indices into `graph.nodes`. `[0]` is the centre; the rest are in order of importance.
    pub nodes: Vec<usize>,
    /// Top, middle, bottom: places in `nodes`, left to right.
    pub rows: [Vec<usize>; 3],
    /// Sorted by `(a, b)`, so the links of the centre come first.
    pub links: Vec<Link>,
    /// How many nodes did not fit in the picture.
    pub left_out: usize,
}

/// `None`: the graph has no concept, so there is nothing to draw.
pub(super) fn plan(graph: &ConceptGraph, focus: Option<ConceptId>) -> Option<Plan> {
    let table = Table::new(graph);
    let centre = table.centre(focus)?;
    let mut others = table.others_by_importance(centre);
    let left_out = others.len().saturating_sub(NODE_CAP - 1);
    others.truncate(NODE_CAP - 1);

    let ids: Vec<NodeId> = std::iter::once(centre).chain(others).collect();
    let rows = table.deal_rows(&ids);
    let place_of: BTreeMap<NodeId, usize> = ids
        .iter()
        .enumerate()
        .map(|(place, id)| (*id, place))
        .collect();
    Some(Plan {
        nodes: ids.iter().map(|id| table.index[id]).collect(),
        rows,
        links: table.links(&place_of),
        left_out,
    })
}

/// True for a matched and for a related concept, false for a result.
pub(super) fn is_concept(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::Concept | NodeKind::Related)
}

/// True when the graph has a concept to put in the centre, so that `plan` gives a plan.
pub(super) fn has_concepts(graph: &ConceptGraph) -> bool {
    graph.nodes.iter().any(|node| is_concept(node.kind))
}

/// What the edges between two nodes say, seen from the node with the lower id.
#[derive(Debug, Default)]
struct Pair {
    kinds: Vec<EdgeKind>,
    points_at_low: bool,
    points_at_high: bool,
}

/// The graph with each id once and each pair of nodes related once. Nothing here reads the
/// order of `graph.nodes` or `graph.edges`, so the same graph in any order gives the same plan.
struct Table<'a> {
    graph: &'a ConceptGraph,
    /// Where each id is in `graph.nodes`.
    index: BTreeMap<NodeId, usize>,
    /// Keyed by the two ids, lower first.
    pairs: BTreeMap<(NodeId, NodeId), Pair>,
    degree: BTreeMap<NodeId, usize>,
}

impl<'a> Table<'a> {
    fn new(graph: &'a ConceptGraph) -> Self {
        // Two nodes with one id: the smaller stays, by kind, then label, then detail. The detail
        // counts too, because it is drawn, and two that differ only there must not be told
        // apart by which came first.
        let mut index: BTreeMap<NodeId, usize> = BTreeMap::new();
        for (place, node) in graph.nodes.iter().enumerate() {
            index
                .entry(node.id)
                .and_modify(|kept| {
                    if node < &graph.nodes[*kept] {
                        *kept = place;
                    }
                })
                .or_insert(place);
        }
        // An edge with an unknown end, or from a node to itself, is dropped.
        let mut pairs: BTreeMap<(NodeId, NodeId), Pair> = BTreeMap::new();
        for edge in &graph.edges {
            if edge.from == edge.to
                || !index.contains_key(&edge.from)
                || !index.contains_key(&edge.to)
            {
                continue;
            }
            let from_is_low = edge.from < edge.to;
            let key = if from_is_low {
                (edge.from, edge.to)
            } else {
                (edge.to, edge.from)
            };
            let pair = pairs.entry(key).or_default();
            if !pair.kinds.contains(&edge.kind) {
                pair.kinds.push(edge.kind);
            }
            if from_is_low {
                pair.points_at_high = true;
            } else {
                pair.points_at_low = true;
            }
        }
        let mut degree: BTreeMap<NodeId, usize> = BTreeMap::new();
        for (low, high) in pairs.keys() {
            *degree.entry(*low).or_default() += 1;
            *degree.entry(*high).or_default() += 1;
        }
        Table {
            graph,
            index,
            pairs,
            degree,
        }
    }

    fn kind(&self, id: NodeId) -> NodeKind {
        self.graph.nodes[self.index[&id]].kind
    }

    fn degree(&self, id: NodeId) -> usize {
        self.degree.get(&id).copied().unwrap_or(0)
    }

    fn lower_case_label(&self, id: NodeId) -> String {
        self.graph.nodes[self.index[&id]].label.to_lowercase()
    }

    fn is_linked(&self, one: NodeId, other: NodeId) -> bool {
        let key = if one < other {
            (one, other)
        } else {
            (other, one)
        };
        self.pairs.contains_key(&key)
    }

    /// The node of this kind with the most links. A tie goes to the lower-case label, then the id.
    fn most_connected(&self, kind: NodeKind) -> Option<NodeId> {
        self.index
            .keys()
            .filter(|id| self.kind(**id) == kind)
            .min_by_key(|id| {
                (
                    Reverse(self.degree(**id)),
                    self.lower_case_label(**id),
                    **id,
                )
            })
            .copied()
    }

    /// The focused concept when the graph has it; else the matched concept with the most links;
    /// else the related concept with the most links.
    fn centre(&self, focus: Option<ConceptId>) -> Option<NodeId> {
        focus
            .map(NodeId::Concept)
            .filter(|id| self.index.contains_key(id))
            .or_else(|| self.most_connected(NodeKind::Concept))
            .or_else(|| self.most_connected(NodeKind::Related))
    }

    /// Every node but the centre: those linked to the centre first, then results, matched
    /// concepts, related concepts; then the most links, the lower-case label and the id.
    fn others_by_importance(&self, centre: NodeId) -> Vec<NodeId> {
        let class = |id: NodeId| match self.kind(id) {
            NodeKind::Formula | NodeKind::Figure | NodeKind::Table => 0,
            NodeKind::Concept => 1,
            NodeKind::Related => 2,
        };
        let mut others: Vec<NodeId> = self
            .index
            .keys()
            .filter(|id| **id != centre)
            .copied()
            .collect();
        others.sort_by_cached_key(|id| {
            (
                !self.is_linked(*id, centre),
                class(*id),
                Reverse(self.degree(*id)),
                self.lower_case_label(*id),
                *id,
            )
        });
        others
    }

    /// The three rows, as places in `ids` (the centre is place 0). Results go to the bottom
    /// row. Concepts are dealt in turn, and a bottom turn is skipped while the bottom row has
    /// fewer nodes than results: those seats are theirs.
    fn deal_rows(&self, ids: &[NodeId]) -> [Vec<usize>; 3] {
        let mut results: Vec<usize> = (1..ids.len())
            .filter(|place| !is_concept(self.kind(ids[*place])))
            .collect();
        results.sort_by_key(|place| (!self.is_linked(ids[*place], ids[0]), ids[*place]));
        let concepts = (1..ids.len()).filter(|place| is_concept(self.kind(ids[*place])));

        let mut dealt: [Vec<usize>; 3] = [Vec::new(), Vec::new(), results.clone()];
        let (mut turns, mut bottom_skipped) = (0, 0);
        for place in concepts {
            let row = loop {
                let row = turn(turns);
                turns += 1;
                if row == BOTTOM && bottom_skipped < results.len() {
                    bottom_skipped += 1;
                    continue;
                }
                break row;
            };
            dealt[row].push(place);
        }
        [
            seat_from_the_middle(VecDeque::new(), &dealt[TOP]),
            seat_from_the_middle(VecDeque::from([0]), &dealt[MIDDLE]),
            seat_from_the_middle(VecDeque::new(), &dealt[BOTTOM]),
        ]
        .map(Vec::from)
    }

    /// One link for each pair of drawn nodes, the centre's first.
    fn links(&self, place_of: &BTreeMap<NodeId, usize>) -> Vec<Link> {
        let mut links: Vec<Link> = self
            .pairs
            .iter()
            .filter_map(|((low, high), pair)| {
                let (low_place, high_place) = (*place_of.get(low)?, *place_of.get(high)?);
                let mut kinds = pair.kinds.clone();
                kinds.sort();
                let (first, second) = if low_place < high_place {
                    (
                        (low_place, pair.points_at_low),
                        (high_place, pair.points_at_high),
                    )
                } else {
                    (
                        (high_place, pair.points_at_high),
                        (low_place, pair.points_at_low),
                    )
                };
                Some(Link {
                    a: first.0,
                    b: second.0,
                    kinds,
                    points_at_a: first.1,
                    points_at_b: second.1,
                })
            })
            .collect();
        links.sort_by_key(|link| (link.a, link.b));
        links
    }
}

/// The row of the concept dealt at this turn: middle, middle, top, bottom, then top, top,
/// bottom, bottom, middle, middle for ever.
fn turn(count: usize) -> usize {
    const OPENING: [usize; 4] = [MIDDLE, MIDDLE, TOP, BOTTOM];
    const AFTER: [usize; 6] = [TOP, TOP, BOTTOM, BOTTOM, MIDDLE, MIDDLE];
    OPENING
        .get(count)
        .copied()
        .unwrap_or_else(|| AFTER[(count - OPENING.len()) % AFTER.len()])
}

/// The first arrival takes the middle, the next goes to its right, the next to its left, and
/// so on. A row that starts with the centre only has arrivals to the right and the left of it.
fn seat_from_the_middle(mut row: VecDeque<usize>, arrivals: &[usize]) -> VecDeque<usize> {
    for place in arrivals {
        if row.is_empty() || row.len() % 2 == 1 {
            row.push_back(*place);
        } else {
            row.push_front(*place);
        }
    }
    row
}

#[cfg(test)]
mod tests {
    use super::super::samples::{black_scholes, c, concept, concept_id, edge, item};
    use super::*;
    use crate::contract::{NodeId, NodeKind};

    fn centre_of(graph: &ConceptGraph, focus: Option<ConceptId>) -> Option<NodeId> {
        let plan = plan(graph, focus)?;
        Some(graph.nodes[plan.nodes[0]].id)
    }

    fn labels<'a>(graph: &'a ConceptGraph, plan: &Plan, row: usize) -> Vec<&'a str> {
        let label = |place: &usize| graph.nodes[plan.nodes[*place]].label.as_str();
        plan.rows[row].iter().map(label).collect()
    }

    #[test]
    fn the_centre_is_the_focused_concept_or_else_the_most_connected_matched_one() {
        let related_has_more = ConceptGraph {
            nodes: vec![
                concept(1, NodeKind::Concept, "Volatility"),
                concept(2, NodeKind::Concept, "Hedging"),
                concept(3, NodeKind::Related, "Smile"),
                concept(4, NodeKind::Concept, "Delta"),
                concept(5, NodeKind::Concept, "Gamma"),
            ],
            edges: vec![
                edge(c(1), c(2), EdgeKind::UsedFor),
                edge(c(1), c(4), EdgeKind::UsedFor),
                edge(c(3), c(1), EdgeKind::UsedFor),
                edge(c(3), c(2), EdgeKind::UsedFor),
                edge(c(3), c(4), EdgeKind::UsedFor),
                edge(c(3), c(5), EdgeKind::UsedFor),
            ],
        };
        // No focus: a matched concept beats a related one that has more links.
        assert_eq!(centre_of(&related_has_more, None), Some(c(1)));
        // A focus that the graph has wins; one that it has not changes nothing.
        let focus = Some(concept_id(2));
        assert_eq!(centre_of(&related_has_more, focus), Some(c(2)));
        let stranger = Some(concept_id(99));
        assert_eq!(centre_of(&related_has_more, stranger), Some(c(1)));

        // A tie is settled by the lower-case name, not by the case of its first letter.
        let tie = ConceptGraph {
            nodes: vec![
                concept(1, NodeKind::Concept, "Zulu"),
                concept(2, NodeKind::Concept, "alpha"),
                concept(3, NodeKind::Concept, "Mike"),
            ],
            edges: vec![
                edge(c(1), c(3), EdgeKind::UsedFor),
                edge(c(2), c(3), EdgeKind::UsedFor),
            ],
        };
        assert_eq!(centre_of(&tie, None), Some(c(3)));
        let pair = ConceptGraph {
            edges: vec![edge(c(1), c(2), EdgeKind::UsedFor)],
            ..tie
        };
        assert_eq!(centre_of(&pair, None), Some(c(2)));

        // With no matched concept, the most connected related one.
        let only_related = ConceptGraph {
            nodes: vec![
                concept(1, NodeKind::Related, "Smile"),
                concept(2, NodeKind::Related, "Skew"),
                concept(3, NodeKind::Related, "Term"),
            ],
            edges: vec![
                edge(c(1), c(2), EdgeKind::UsedFor),
                edge(c(3), c(2), EdgeKind::UsedFor),
            ],
        };
        assert_eq!(centre_of(&only_related, None), Some(c(2)));

        // No concept at all: nothing to draw.
        let only_items = ConceptGraph {
            nodes: vec![item(1, NodeKind::Formula, "Formula (1.1)")],
            edges: Vec::new(),
        };
        assert_eq!(centre_of(&only_items, None), None);
        assert_eq!(centre_of(&ConceptGraph::default(), None), None);
    }

    #[test]
    fn the_wireframe_graph_gets_the_wireframe_rows_and_one_link_for_each_pair() {
        let graph = black_scholes();
        let plan = plan(&graph, None).expect("the graph has concepts");

        assert_eq!(graph.nodes[plan.nodes[0]].label, "Black–Scholes model");
        assert_eq!(plan.left_out, 0);
        assert_eq!(
            labels(&graph, &plan, TOP),
            ["Itô's lemma", "Risk-neutral measure"]
        );
        assert_eq!(
            labels(&graph, &plan, MIDDLE),
            [
                "Geometric Brownian motion",
                "Black–Scholes model",
                "European option"
            ]
        );
        assert_eq!(
            labels(&graph, &plan, BOTTOM),
            ["Formula (3.17)", "Figure 3.1"]
        );

        // Seven edges were stated, one of them twice: six links, all of the centre.
        assert_eq!(plan.links.len(), 6);
        assert!(plan.links.iter().all(|link| link.a == 0));
        let derived = plan
            .links
            .iter()
            .find(|link| graph.nodes[plan.nodes[link.b]].label == "Risk-neutral measure")
            .expect("the model is linked to the measure");
        assert_eq!(derived.kinds, [EdgeKind::DerivedFrom]);
        // "Black–Scholes derived from Risk-neutral" points at the measure, not at the model.
        assert!(derived.points_at_b && !derived.points_at_a);
        let part_of = plan
            .links
            .iter()
            .find(|link| graph.nodes[plan.nodes[link.b]].label == "European option")
            .expect("the model is linked to the option");
        assert!(part_of.points_at_a && !part_of.points_at_b);
    }
}
