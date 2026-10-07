//! Where everything is: a rectangle for each node, a route and a label place for each link, and
//! the size of the whole picture. Plain numbers in, plain numbers out.

use super::geometry::{Point, Rect, Route, point};
use super::plan::Plan;
use super::seats::{Layout, Metrics, Sizes};

/// The space kept clear between two labels that show at rest. Do not raise it above 5.75: in a
/// panel of the default size a label over a short link and a label in the band above it stand
/// that far apart, and with more clearance one of the two would not show.
const LABEL_CLEARANCE: f32 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct LabelPlace {
    pub rect: Rect,
    /// Drawn with no pointer over it: a link of the centre whose label found a free place.
    pub at_rest: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Placed {
    /// The whole picture. The centre node's centre line is `x = 0`, and it is never smaller
    /// than the canvas.
    pub world: Rect,
    /// One for each of `Plan::nodes`.
    pub nodes: Vec<Rect>,
    /// One for each of `Plan::links`.
    pub routes: Vec<Route>,
    /// One for each of `Plan::links`.
    pub labels: Vec<LabelPlace>,
}

/// Gives every node of the plan its rectangle, and every link its route and the place of its
/// words.
pub(super) fn place(plan: &Plan, sizes: &Sizes<'_>, metrics: &Metrics) -> Placed {
    let layout = Layout::new(plan, sizes, metrics);
    let world = layout.world();
    let (routes, spots) = layout.routes_and_label_spots();
    let labels = spots
        .into_iter()
        .map(|rect| LabelPlace {
            rect,
            at_rest: false,
        })
        .collect();
    let mut placed = Placed {
        world,
        nodes: layout.nodes,
        routes,
        labels,
    };
    let rest_places = placed.rest_places(plan);
    for (label, rest) in placed.labels.iter_mut().zip(rest_places) {
        if let Some(rect) = rest {
            *label = LabelPlace {
                rect,
                at_rest: true,
            };
        }
    }
    placed
}

impl Placed {
    /// For each link, the place where its label shows at rest, or `None`. Only the links of the
    /// centre show a label at rest, in order, and only where nothing is in the way: a place is
    /// taken when it is inside the picture, holds no node, is clear of every label already
    /// taken, and no link of the centre crosses it, the label's own link included. Else the
    /// mirror image of the place about the middle of the route is tried.
    fn rest_places(&self, plan: &Plan) -> Vec<Option<Rect>> {
        let mut places = vec![None; plan.links.len()];
        let mut taken: Vec<Rect> = Vec::new();
        for (index, link) in plan.links.iter().enumerate() {
            if !link.touches(0) {
                continue;
            }
            let rect = self.labels[index].rect;
            let mirrored = Rect::from_centre(
                point(
                    2.0 * middle_of(self.routes[index]).x - rect.centre().x,
                    rect.centre().y,
                ),
                rect.size(),
            );
            let is_free = |candidate: &Rect| {
                self.world.holds(candidate)
                    && !self.nodes.iter().any(|node| node.overlaps(candidate))
                    && !taken
                        .iter()
                        .any(|other| other.inflate(LABEL_CLEARANCE).overlaps(candidate))
                    && !plan.links.iter().zip(&self.routes).any(|(other, route)| {
                        other.touches(0)
                            && route
                                .pieces()
                                .windows(2)
                                .any(|pair| candidate.cut_by(pair[0], pair[1]))
                    })
            };
            if let Some(found) = [rect, mirrored].into_iter().find(|rect| is_free(rect)) {
                places[index] = Some(found);
                taken.push(found);
            }
        }
        places
    }
}

/// The point half way along a route.
fn middle_of(route: Route) -> Point {
    let pieces = route.pieces();
    let half = pieces.len() / 2;
    if pieces.len() % 2 == 1 {
        pieces[half]
    } else {
        let (before, after) = (pieces[half - 1], pieces[half]);
        point((before.x + after.x) / 2.0, (before.y + after.y) / 2.0)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use proptest::prelude::*;
    use uuid::Uuid;

    use super::super::geometry::Size;
    use super::super::plan::plan;
    use super::super::samples::graphs;
    use super::*;
    use crate::contract::{ConceptGraph, ConceptId, NodeId, NodeKind};

    fn metrics(canvas: Size) -> Metrics {
        let roomy = canvas.h >= 168.0;
        Metrics {
            pill_height: if roomy { 40.0 } else { 28.0 },
            gap: 16.0,
            label_pad: if roomy { 12.0 } else { 8.0 },
            band_min: 16.0,
            band_max: 44.0,
            margin: 4.0,
            corner: 12.0,
            label_height: 16.0,
        }
    }

    /// One draw of the property: a graph, a focus, a canvas, and a salt for the widths.
    #[derive(Debug, Clone)]
    struct Case {
        graph: ConceptGraph,
        focus: Option<ConceptId>,
        canvas: Size,
        seed: u32,
    }

    prop_compose! {
        fn cases()(
            graph in graphs(),
            focus in prop::option::of(0u8..70),
            w in 300.0f32..1800.0,
            h in 100.0f32..320.0,
            seed in any::<u32>(),
        ) -> Case {
            let focus = focus.map(|number| ConceptId(Uuid::from_u128(u128::from(number))));
            Case { graph, focus, canvas: Size { w, h }, seed }
        }
    }

    impl Case {
        /// The plan and the places. Widths depend on the node alone, never on its place in
        /// the list.
        fn laid_out(&self) -> Option<(Plan, Placed)> {
            let plan = plan(&self.graph, self.focus)?;
            let width = |id: NodeId| {
                let salt = match id {
                    NodeId::Concept(ConceptId(uuid)) => uuid.as_u128() as u32,
                    NodeId::Item(number) => number as u32 + 1000,
                };
                let mixed = salt.wrapping_mul(2_654_435_761).wrapping_add(self.seed);
                48.0 + (mixed % 150) as f32
            };
            let nodes: Vec<f32> = plan
                .nodes
                .iter()
                .map(|index| width(self.graph.nodes[*index].id))
                .collect();
            let labels: Vec<f32> = plan
                .links
                .iter()
                .map(|link| 30.0 + 14.0 * link.kinds.len() as f32)
                .collect();
            let sizes = Sizes {
                nodes: &nodes,
                labels: &labels,
                canvas: self.canvas,
            };
            let placed = place(&plan, &sizes, &metrics(self.canvas));
            Some((plan, placed))
        }

        /// The same case with the nodes and the edges in another order.
        fn shuffled(&self) -> Case {
            let mut graph = self.graph.clone();
            graph.nodes.reverse();
            let turn = self.seed as usize % graph.nodes.len().max(1);
            graph.nodes.rotate_left(turn);
            graph.edges.reverse();
            Case {
                graph,
                ..self.clone()
            }
        }
    }

    /// A picture told by id, so that two of them compare whatever order the graph came in.
    #[derive(Debug, PartialEq)]
    struct ById {
        /// The corners of each node, in sixteenths of a point.
        rects: BTreeMap<NodeId, [i32; 4]>,
        /// The two ends, the route and the kinds of each link.
        routes: Vec<(NodeId, NodeId, String)>,
    }

    fn by_id(graph: &ConceptGraph, plan: &Plan, placed: &Placed) -> ById {
        let id = |place: usize| graph.nodes[plan.nodes[place]].id;
        let rects = placed
            .nodes
            .iter()
            .enumerate()
            .map(|(place, rect)| {
                let sixteenths = [rect.min.x, rect.min.y, rect.max.x, rect.max.y]
                    .map(|value| (value * 16.0).round() as i32);
                (id(place), sixteenths)
            })
            .collect();
        let mut routes: Vec<(NodeId, NodeId, String)> = plan
            .links
            .iter()
            .zip(&placed.routes)
            .map(|(link, route)| {
                (
                    id(link.a),
                    id(link.b),
                    format!("{route:?} {:?}", link.kinds),
                )
            })
            .collect();
        routes.sort();
        ById { rects, routes }
    }

    fn nodes_stand_clear(placed: &Placed, canvas: Size) -> Result<(), TestCaseError> {
        for (place, node) in placed.nodes.iter().enumerate() {
            prop_assert!(
                placed.world.holds(node),
                "node {place} leaves the picture: {node:?} in {:?}",
                placed.world
            );
            for other in &placed.nodes[place + 1..] {
                prop_assert!(!node.overlaps(other), "{node:?} overlaps {other:?}");
            }
        }
        prop_assert!(
            placed.world.width() >= canvas.w - 0.01 && placed.world.height() >= canvas.h - 0.01
        );
        prop_assert!(placed.nodes[0].centre().x.abs() < 0.01);
        Ok(())
    }

    fn no_route_cuts_a_node(plan: &Plan, placed: &Placed) -> Result<(), TestCaseError> {
        for (link, route) in plan.links.iter().zip(&placed.routes) {
            let pieces = route.pieces();
            for (place, node) in placed.nodes.iter().enumerate() {
                if link.touches(place) {
                    continue;
                }
                let inside = node.inflate(-0.5);
                for pair in pieces.windows(2) {
                    prop_assert!(
                        !inside.cut_by(pair[0], pair[1]),
                        "{route:?} of {link:?} cuts node {place} {node:?}"
                    );
                }
            }
        }
        Ok(())
    }

    fn rest_labels_are_free(plan: &Plan, placed: &Placed) -> Result<(), TestCaseError> {
        let resting: Vec<Rect> = placed
            .labels
            .iter()
            .filter(|label| label.at_rest)
            .map(|label| label.rect)
            .collect();
        for (turn, rect) in resting.iter().enumerate() {
            prop_assert!(placed.world.holds(rect));
            prop_assert!(!placed.nodes.iter().any(|node| node.overlaps(rect)));
            prop_assert!(!resting[turn + 1..].iter().any(|other| rect.overlaps(other)));
            // No link of the centre runs through a label, and that holds for the label's own link.
            for (link, route) in plan.links.iter().zip(&placed.routes) {
                if link.touches(0) {
                    let pieces = route.pieces();
                    prop_assert!(
                        !pieces.windows(2).any(|pair| rect.cut_by(pair[0], pair[1])),
                        "{route:?} runs through the label at {rect:?}"
                    );
                }
            }
        }
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 256,
            failure_persistence: None,
            ..ProptestConfig::default()
        })]

        #[test]
        fn a_layout_is_clean_and_does_not_depend_on_the_order_it_was_given(case in cases()) {
            let Some((plan, placed)) = case.laid_out() else {
                let has_concept = case.graph.nodes.iter().any(|node| {
                    matches!(node.kind, NodeKind::Concept | NodeKind::Related)
                });
                prop_assert!(!has_concept);
                return Ok(());
            };
            nodes_stand_clear(&placed, case.canvas)?;
            no_route_cuts_a_node(&plan, &placed)?;
            rest_labels_are_free(&plan, &placed)?;

            let shuffled = case.shuffled();
            let (plan_again, placed_again) = shuffled
                .laid_out()
                .expect("the same graph has the same centre");
            prop_assert_eq!(
                by_id(&case.graph, &plan, &placed),
                by_id(&shuffled.graph, &plan_again, &placed_again)
            );
        }
    }
}
