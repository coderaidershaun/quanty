//! Where each link goes and where its label stands, once every node has its place.

use super::geometry::{Point, Rect, Route, Size, point};
use super::plan::{MIDDLE, TOP};
use super::seats::Layout;

/// How far a label stands above a short line.
const LABEL_LIFT: f32 = 3.0;
/// How far a label stands to the side of a link that crosses a band.
const LABEL_SIDE: f32 = 6.0;
/// A route this close to the centre line, or nearer, counts as on it.
const CENTRE_LINE_ZONE: f32 = 4.0;
/// How far a straight link must stay from a node of the middle row, and how far a bent one
/// goes beyond it.
const DETOUR_CLEARANCE: f32 = 3.0;
/// A curve between two nodes of one row rises 8 points, and 3 more for each 100 points between
/// them, but never so far that it leaves its band.
const ARC_RISE: f32 = 8.0;
const ARC_RISE_PER_POINT: f32 = 0.03;
const ARC_RISE_MIN: f32 = 3.0;
/// A curve stays this far inside its band.
const ARC_BAND_MARGIN: f32 = 4.0;
/// Two nodes of the middle row with a gap up to this much wider than the plain gap are still
/// side by side with nothing in between.
const GAP_TOLERANCE: f32 = 0.5;
/// A curve starts and ends this part of a node's width to the side of the node's centre.
const ARC_FOOT: f32 = 0.25;

/// The two ends of one link as they stand.
struct Ends {
    a: Rect,
    b: Rect,
    row_a: usize,
    row_b: usize,
    /// How many seats apart the two ends are, when they are in one row.
    seats_apart: usize,
}

impl Ends {
    fn a_is_left(&self) -> bool {
        self.a.min.x < self.b.min.x
    }

    fn a_is_upper(&self) -> bool {
        self.a.min.y < self.b.min.y
    }

    /// The end that stands left, and the end that stands right.
    fn left_right(&self) -> (Rect, Rect) {
        if self.a_is_left() {
            (self.a, self.b)
        } else {
            (self.b, self.a)
        }
    }

    /// The end that stands higher, and the end that stands lower.
    fn upper_lower(&self) -> (Rect, Rect) {
        if self.a_is_upper() {
            (self.a, self.b)
        } else {
            (self.b, self.a)
        }
    }

    /// `route` was drawn from the left end. Turn it to run from `a` to `b`.
    fn running_from_a(&self, route: Route) -> Route {
        if self.a_is_left() {
            route
        } else {
            route.reversed()
        }
    }

    /// `route` was drawn from the upper end. Turn it to run from `a` to `b`.
    fn running_down_from_a(&self, route: Route) -> Route {
        if self.a_is_upper() {
            route
        } else {
            route.reversed()
        }
    }
}

impl Layout<'_> {
    /// The route of each link and the place of its label, in the order of `Plan::links`.
    pub(super) fn routes_and_label_spots(&self) -> (Vec<Route>, Vec<Rect>) {
        (0..self.plan.links.len())
            .map(|index| self.route(&self.ends(index), self.label_size(index)))
            .unzip()
    }

    fn label_size(&self, index: usize) -> Size {
        Size {
            w: self.sizes.labels[index],
            h: self.metrics.label_height,
        }
    }

    fn ends(&self, index: usize) -> Ends {
        let link = &self.plan.links[index];
        Ends {
            a: self.nodes[link.a],
            b: self.nodes[link.b],
            row_a: self.row_of[link.a],
            row_b: self.row_of[link.b],
            seats_apart: self.seat_of[link.a].abs_diff(self.seat_of[link.b]),
        }
    }

    /// The height of the middle of the band under `upper_row`.
    fn band_middle(&self, upper_row: usize) -> f32 {
        self.rows.top[upper_row] + self.metrics.pill_height + self.rows.band / 2.0
    }

    /// The route of a link and the place of its label. Every route stays inside a band or a
    /// gap, so it never crosses a node.
    fn route(&self, ends: &Ends, label: Size) -> (Route, Rect) {
        if ends.row_a == ends.row_b {
            return self.same_row(ends, label);
        }
        let upper_row = ends.row_a.min(ends.row_b);
        let route = if ends.row_a.abs_diff(ends.row_b) == 1 {
            self.between_rows(ends)
        } else {
            self.across_the_middle(ends)
        };
        (route, beside(route, self.band_middle(upper_row), label))
    }

    /// Two nodes of one row: a short line when they stand side by side, else a curve through
    /// the band next to the row, which holds no node.
    fn same_row(&self, ends: &Ends, label: Size) -> (Route, Rect) {
        let (left, right) = ends.left_right();
        let (rows, metrics) = (&self.rows, self.metrics);
        let inward = if ends.row_a == TOP { 1.0 } else { -1.0 };
        let edge_y = if ends.row_a == TOP {
            left.max.y
        } else {
            left.min.y
        };
        let band_centre = edge_y + inward * rows.band / 2.0;
        if ends.seats_apart == 1 {
            let y = left.centre().y;
            let line = Route::Line(point(left.max.x, y), point(right.min.x, y));
            let mid_x = (left.max.x + right.min.x) / 2.0;
            let has_room = right.min.x - left.max.x >= label.w + metrics.label_pad;
            let label_y = if has_room {
                y - LABEL_LIFT - label.h / 2.0
            } else {
                band_centre
            };
            let spot = Rect::from_centre(point(mid_x, label_y), label);
            return (ends.running_from_a(line), spot);
        }
        let from = point(left.centre().x + left.width() * ARC_FOOT, edge_y);
        let to = point(right.centre().x - right.width() * ARC_FOOT, edge_y);
        let rise = (rows.band - ARC_BAND_MARGIN)
            .min(ARC_RISE + ARC_RISE_PER_POINT * (to.x - from.x))
            .max(ARC_RISE_MIN);
        let control = point((from.x + to.x) / 2.0, edge_y + inward * 2.0 * rise);
        let arc = Route::Arc(from, control, to);
        let spot = Rect::from_centre(point(control.x, band_centre), label);
        (ends.running_from_a(arc), spot)
    }

    /// Two nodes of rows next to each other: a straight line through the band between them.
    fn between_rows(&self, ends: &Ends) -> Route {
        let (upper, lower) = ends.upper_lower();
        let (from, to) = self.feet(upper, lower);
        ends.running_down_from_a(Route::Line(from, to))
    }

    /// Where a straight link leaves the upper node and reaches the lower one: each at the
    /// x of the other node's centre, kept away from the corners.
    fn feet(&self, upper: Rect, lower: Rect) -> (Point, Point) {
        let corner = self.metrics.corner;
        let from = point(clamp_into(lower.centre().x, upper, corner), upper.max.y);
        let to = point(clamp_into(upper.centre().x, lower, corner), lower.min.y);
        (from, to)
    }

    /// A top node and a bottom node: straight when no middle node is in the way, else down to
    /// the gap of the middle row with the shortest detour, through it, and on to the end.
    fn across_the_middle(&self, ends: &Ends) -> Route {
        let (upper, lower) = ends.upper_lower();
        let middle: Vec<Rect> = self.plan.rows[MIDDLE]
            .iter()
            .map(|place| self.nodes[*place])
            .collect();
        let (from, to) = self.feet(upper, lower);
        if !middle
            .iter()
            .any(|node| node.inflate(DETOUR_CLEARANCE).cut_by(from, to))
        {
            return ends.running_down_from_a(Route::Line(from, to));
        }
        let gap = self.metrics.gap;
        let plain_gaps = middle
            .windows(2)
            .filter(|pair| pair[1].min.x - pair[0].max.x <= gap + GAP_TOLERANCE)
            .map(|pair| (pair[0].max.x + pair[1].min.x) / 2.0);
        let beyond_the_right = middle
            .iter()
            .map(|node| node.max.x)
            .fold(f32::NEG_INFINITY, f32::max)
            + gap;
        let beyond_the_left = middle
            .iter()
            .map(|node| node.min.x)
            .fold(f32::INFINITY, f32::min)
            - gap;
        let detour = |gate: f32| (upper.centre().x - gate).abs() + (lower.centre().x - gate).abs();
        let shorter = |gate: f32, best: f32| {
            detour(gate)
                .total_cmp(&detour(best))
                .then(gate.abs().total_cmp(&best.abs()))
                .then(gate.total_cmp(&best))
                .is_lt()
        };
        let gate = plain_gaps
            .chain([beyond_the_right])
            .fold(beyond_the_left, |best, gate| {
                if shorter(gate, best) { gate } else { best }
            });
        let corner = self.metrics.corner;
        let (row_top, row_bottom) = (middle[0].min.y, middle[0].max.y);
        let bent = Route::Bent([
            point(clamp_into(gate, upper, corner), upper.max.y),
            point(gate, row_top - DETOUR_CLEARANCE),
            point(gate, row_bottom + DETOUR_CLEARANCE),
            point(clamp_into(gate, lower, corner), lower.min.y),
        ]);
        ends.running_down_from_a(bent)
    }
}

fn clamp_into(x: f32, rect: Rect, corner: f32) -> f32 {
    let (low, high) = (rect.min.x + corner, rect.max.x - corner);
    if low > high {
        rect.centre().x
    } else {
        x.clamp(low, high)
    }
}

/// A label beside the first piece of a route that crosses a band, at the band's mid height, on
/// the side away from the centre line so that labels do not crowd the centre node. It stands
/// clear of the piece over its whole height, so a link that leans far does not run through its
/// own label.
fn beside(route: Route, band_middle: f32, label: Size) -> Rect {
    let pieces = route.pieces();
    let last = pieces.len() - 1;
    let (first, second) = if pieces[0].y < pieces[last].y {
        (pieces[0], pieces[1])
    } else {
        (pieces[last], pieces[last - 1])
    };
    let lean = second.x - first.x;
    let x_at = |y: f32| {
        // A level piece has no height to measure along, so take its middle.
        let along = if (second.y - first.y).abs() < f32::EPSILON {
            0.5
        } else {
            (y - first.y) / (second.y - first.y)
        };
        first.x + lean * along.clamp(0.0, 1.0)
    };
    let (top, bottom) = (band_middle - label.h / 2.0, band_middle + label.h / 2.0);
    let left = if x_at(band_middle) < -CENTRE_LINE_ZONE {
        x_at(top).min(x_at(bottom)) - LABEL_SIDE - label.w
    } else {
        x_at(top).max(x_at(bottom)) + LABEL_SIDE
    };
    Rect::from_min_size(point(left, top), label)
}
