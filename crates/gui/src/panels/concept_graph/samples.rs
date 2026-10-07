//! What the tests of this panel draw with: the graphs (the picture of the design, one node, and a
//! graph that is too big to draw whole, all the same every time) and a window around the panel.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use proptest::prelude::*;
use uuid::Uuid;

use super::{Local, show};
use crate::contract::{ConceptGraph, ConceptId, EdgeKind, GraphEdge, GraphNode, NodeId, NodeKind};
use crate::state::Shared;
use crate::testkit::{self, Host, sample};
use crate::theme::space;

/// The outer size of the panel at the default window, and at the smallest.
pub(super) const DEFAULT: [f32; 2] = [556.0, 273.0];
pub(super) const SMALLEST: [f32; 2] = [401.0, 220.0];
/// The harness puts the panel inside a gutter of this many points.
const GUTTER: f32 = space::LG;

/// The panel alone in a window, and its own state.
pub(super) fn drawn(
    size: [f32; 2],
    shared: Shared,
) -> (Harness<'static, Host>, Rc<RefCell<Local>>) {
    let local = Rc::new(RefCell::new(Local::default()));
    let shown = Rc::clone(&local);
    let harness = testkit::panel(size, shared, move |ui, cx| {
        show(ui, &mut shown.borrow_mut(), cx);
    });
    (harness, local)
}

/// The state after the search, the graph and the answer came back.
pub(super) fn answered(graph: ConceptGraph) -> Shared {
    testkit::answered(sample::search_reply(), graph, sample::answer())
}

/// Writes `<QUANTY_PNG_DIR>/<name>.png` as `testkit::save_png` does, but leaves the pointer where
/// it is, so that the picture shows what a hover does. Does nothing when the variable is not set.
pub(super) fn save_png_with_pointer(harness: &mut Harness<'_, Host>, name: &str) {
    let Some(folder) = std::env::var_os("QUANTY_PNG_DIR") else {
        return;
    };
    let image = harness.render().expect("the test renderer draws the frame");
    let folder = PathBuf::from(folder);
    std::fs::create_dir_all(&folder).expect("the folder for the pictures can be made");
    image
        .save(folder.join(format!("{name}.png")))
        .expect("the picture is written");
}

/// The nodes that the panel draws, in the order of the plan: the centre first.
pub(super) fn drawn_nodes<'a>(
    local: &RefCell<Local>,
    graph: &'a ConceptGraph,
) -> Vec<&'a GraphNode> {
    let local = local.borrow();
    let scene = local.scene.as_ref().expect("the graph has a scene");
    let places = scene.plan.nodes.iter();
    places.map(|index| &graph.nodes[*index]).collect()
}

/// How wide the node at `place` is when all of it is in sight. The rectangle that a test reads
/// from a node is only the part of the node that is inside the canvas.
pub(super) fn whole_width(local: &RefCell<Local>, place: usize) -> f32 {
    let local = local.borrow();
    let scene = local.scene.as_ref().expect("the graph has a scene");
    scene.placed.nodes[place].width() * local.view.scale()
}

/// Where the panel stands in the window of the harness.
pub(super) fn panel_rect(size: [f32; 2]) -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(GUTTER, GUTTER), egui::Vec2::from(size))
}

/// A point of the canvas at the default size where there is no node: left of the bottom row.
pub(super) fn an_empty_place() -> egui::Pos2 {
    panel_rect(DEFAULT).left_bottom() + egui::vec2(24.0, -20.0)
}

pub(super) fn click_at(harness: &mut Harness<'_, Host>, at: egui::Pos2) {
    harness.hover_at(at);
    harness.run_ok();
    harness.drag_at(at);
    harness.run_ok();
    harness.drop_at(at);
    harness.run_ok();
}

pub(super) fn is_marked(harness: &Harness<'_, Host>, name: &str) -> bool {
    let toggled = harness.get_by_label(name).accesskit_node().toggled();
    toggled == Some(egui::accesskit::Toggled::True)
}

pub(super) fn wheel(harness: &Harness<'_, Host>, delta: egui::Vec2) {
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta,
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    });
}

pub(super) fn concept_id(number: u128) -> ConceptId {
    ConceptId(Uuid::from_u128(number))
}

/// The concept with this number, as a node id.
pub(super) fn c(number: u128) -> NodeId {
    NodeId::Concept(concept_id(number))
}

pub(super) fn concept(number: u128, kind: NodeKind, label: &str) -> GraphNode {
    GraphNode {
        id: c(number),
        kind,
        label: label.to_owned(),
        detail: Some(format!("What the books say {label} is.")),
    }
}

pub(super) fn item(number: usize, kind: NodeKind, label: &str) -> GraphNode {
    GraphNode {
        id: NodeId::Item(number),
        kind,
        label: label.to_owned(),
        detail: None,
    }
}

/// The same node with a detail: a result's name or caption.
pub(super) fn described(node: GraphNode, detail: &str) -> GraphNode {
    GraphNode {
        detail: Some(detail.to_owned()),
        ..node
    }
}

pub(super) fn edge(from: NodeId, to: NodeId, kind: EdgeKind) -> GraphEdge {
    GraphEdge { from, to, kind }
}

/// The graph of the design: one concept in the middle with two concepts and an option beside
/// it, two related concepts above, a formula and a figure below. The link between the model and
/// the risk-neutral measure is stated twice.
pub(super) fn black_scholes() -> ConceptGraph {
    ConceptGraph {
        nodes: vec![
            concept(1, NodeKind::Concept, "Black–Scholes model"),
            concept(2, NodeKind::Concept, "Geometric Brownian motion"),
            concept(3, NodeKind::Concept, "European option"),
            concept(4, NodeKind::Related, "Risk-neutral measure"),
            concept(5, NodeKind::Related, "Itô's lemma"),
            described(
                item(1, NodeKind::Formula, "Formula (3.17)"),
                "Black–Scholes formula",
            ),
            described(
                item(8, NodeKind::Figure, "Figure 3.1"),
                "Normal distribution N(d₁)",
            ),
        ],
        edges: vec![
            edge(c(1), c(2), EdgeKind::Assumes),
            edge(c(1), c(4), EdgeKind::DerivedFrom),
            edge(c(3), c(1), EdgeKind::PartOf),
            edge(c(5), c(1), EdgeKind::Generalises),
            edge(NodeId::Item(1), c(1), EdgeKind::Mentions),
            edge(NodeId::Item(8), c(1), EdgeKind::Mentions),
            edge(c(1), c(4), EdgeKind::DerivedFrom),
        ],
    }
}

/// A graph of one concept and nothing else.
pub(super) fn one_node() -> ConceptGraph {
    ConceptGraph {
        nodes: vec![concept(1, NodeKind::Concept, "Black–Scholes model")],
        edges: Vec::new(),
    }
}

const NAMES: [&str; 12] = [
    "Volatility",
    "Hedging",
    "European option",
    "Risk-free rate",
    "Option premium",
    "Delta",
    "Put–call parity",
    "Implied volatility",
    "Binomial model",
    "Arbitrage",
    "Martingale",
    "Dividend yield",
];

/// The kinds of the nodes in turn: concepts first, then one or two of every other kind.
const KINDS: [NodeKind; 9] = [
    NodeKind::Concept,
    NodeKind::Concept,
    NodeKind::Concept,
    NodeKind::Related,
    NodeKind::Related,
    NodeKind::Formula,
    NodeKind::Formula,
    NodeKind::Figure,
    NodeKind::Table,
];

const CONCEPT_LINKS: [EdgeKind; 5] = [
    EdgeKind::DerivedFrom,
    EdgeKind::Assumes,
    EdgeKind::Generalises,
    EdgeKind::PartOf,
    EdgeKind::UsedFor,
];

/// A graph of `count` nodes of every kind. Each concept but the first hangs on an earlier one,
/// and each result mentions two concepts.
pub(super) fn dense(count: usize) -> ConceptGraph {
    let mut nodes = Vec::with_capacity(count);
    let mut concepts: Vec<NodeId> = Vec::new();
    let mut results: Vec<NodeId> = Vec::new();
    for place in 0..count {
        let kind = KINDS[place % KINDS.len()];
        let name = NAMES[place % NAMES.len()];
        let number = place as u128 + 1;
        let node = match kind {
            NodeKind::Concept | NodeKind::Related => {
                let label = if place < NAMES.len() {
                    name.to_owned()
                } else {
                    format!("{name} {place}")
                };
                concept(number, kind, &label)
            }
            NodeKind::Formula => described(
                item(place + 1, kind, &format!("Formula (2.{place})")),
                &format!("The formula of {name}"),
            ),
            NodeKind::Figure => described(
                item(place + 1, kind, &format!("Figure 3.{place}")),
                &format!("A figure of {name}"),
            ),
            NodeKind::Table => described(
                item(place + 1, kind, &format!("Table 1-{place}")),
                &format!("A table of {name}"),
            ),
        };
        if matches!(kind, NodeKind::Concept | NodeKind::Related) {
            concepts.push(node.id);
        } else {
            results.push(node.id);
        }
        nodes.push(node);
    }
    let mut edges = Vec::new();
    for (turn, concept) in concepts.iter().enumerate().skip(1) {
        let kind = CONCEPT_LINKS[turn % CONCEPT_LINKS.len()];
        let parent = concepts[turn / 2];
        edges.push(if turn % 2 == 0 {
            edge(*concept, parent, kind)
        } else {
            edge(parent, *concept, kind)
        });
        let cousin = concepts[turn * 2 / 3];
        if turn % 2 == 0 && cousin != parent {
            edges.push(edge(*concept, cousin, kind));
        }
    }
    for (turn, result) in results.iter().enumerate() {
        for mentioned in [turn % concepts.len(), (turn * 5 + 1) % concepts.len()] {
            edges.push(edge(*result, concepts[mentioned], EdgeKind::Mentions));
        }
    }
    ConceptGraph { nodes, edges }
}

const NODE_KINDS: [NodeKind; 5] = [
    NodeKind::Concept,
    NodeKind::Related,
    NodeKind::Formula,
    NodeKind::Figure,
    NodeKind::Table,
];
const EDGE_KINDS: [EdgeKind; 6] = [
    EdgeKind::DerivedFrom,
    EdgeKind::Assumes,
    EdgeKind::Generalises,
    EdgeKind::PartOf,
    EdgeKind::UsedFor,
    EdgeKind::Mentions,
];

fn id_of(number: u8, kind: NodeKind) -> NodeId {
    match kind {
        NodeKind::Concept | NodeKind::Related => c(u128::from(number)),
        _ => NodeId::Item(usize::from(number)),
    }
}

/// A node at this place or later takes the number of an earlier node, so that an id can come
/// twice, with another kind and another label.
const REPEATS_FROM: usize = 50;

prop_compose! {
    /// Up to 60 nodes of any kind, the last ten with the number of an earlier node, and up to
    /// 140 edges: repeats, links to self and edges to a node that is not there.
    pub(super) fn graphs()(
        kinds in prop::collection::vec(0..NODE_KINDS.len(), 0..60),
        edges in prop::collection::vec((0u8..70, 0u8..70, 0..EDGE_KINDS.len()), 0..140),
    ) -> ConceptGraph {
        let nodes: Vec<GraphNode> = kinds
            .iter()
            .enumerate()
            .map(|(number, code)| {
                let kind = NODE_KINDS[*code];
                GraphNode {
                    id: id_of((number % REPEATS_FROM) as u8, kind),
                    kind,
                    label: format!("n{}", number % 7),
                    detail: None,
                }
            })
            .collect();
        let pick = |number: u8| {
            nodes
                .get(usize::from(number))
                .map_or(NodeId::Item(999), |node| node.id)
        };
        let edges = edges
            .iter()
            .map(|(from, to, kind)| GraphEdge {
                from: pick(*from),
                to: pick(*to),
                kind: EDGE_KINDS[*kind],
            })
            .collect();
        ConceptGraph { nodes, edges }
    }
}
