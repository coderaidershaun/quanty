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
    /// There is one for each of `Plan::nodes`.
    pub nodes: Vec<Rect>,
    /// There is one for each of `Plan::links`.
    pub routes: Vec<Route>,
    /// There is one for each of `Plan::links`.
    pub labels: Vec<LabelPlace>,
}

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
    /// Only the links of the centre show a label at rest, and only where nothing is in the way;
    /// a label's own link counts as in the way too. Else the mirror image of the place about the
    /// middle of the route is tried.
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
