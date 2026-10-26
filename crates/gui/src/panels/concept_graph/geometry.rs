//! Link routes and the two tests on rectangles that the layout needs, so that it can be worked out
//! without a window.

use eframe::egui::{Pos2, Rect, Vec2, pos2};

/// The rectangle of `size` around `centre`, worked out from its corner so that it is the same to
/// the last bit as the rectangles the layout builds from a corner.
pub(super) fn centred(centre: Pos2, size: Vec2) -> Rect {
    Rect::from_min_size(centre - size * 0.5, size)
}

/// Touching sides do not overlap.
pub(super) fn overlaps(a: &Rect, b: &Rect) -> bool {
    a.min.x < b.max.x && b.min.x < a.max.x && a.min.y < b.max.y && b.min.y < a.max.y
}

/// True when the segment from `from` to `to` passes through the inside of the rectangle.
/// Clips the segment to each of the four sides in turn.
pub(super) fn cut_by(rect: &Rect, from: Pos2, to: Pos2) -> bool {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let (mut enters, mut leaves) = (0.0_f32, 1.0_f32);
    let sides = [
        (-dx, from.x - rect.min.x),
        (dx, rect.max.x - from.x),
        (-dy, from.y - rect.min.y),
        (dy, rect.max.y - from.y),
    ];
    for (towards, room) in sides {
        if towards == 0.0 {
            if room <= 0.0 {
                return false;
            }
            continue;
        }
        let at = room / towards;
        if towards < 0.0 {
            enters = enters.max(at);
        } else {
            leaves = leaves.min(at);
        }
    }
    enters < leaves
}

const CURVE_PIECES: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Route {
    Line(Pos2, Pos2),
    /// The three points are the start, the control and the end of a quadratic curve.
    Arc(Pos2, Pos2, Pos2),
    /// The route goes down a band, through a gap of the middle row, and down the next band.
    Bent([Pos2; 4]),
}

impl Route {
    /// The route as straight pieces: the points between them, in order.
    pub(super) fn pieces(&self) -> Vec<Pos2> {
        match *self {
            Route::Line(from, to) => vec![from, to],
            Route::Bent(points) => points.to_vec(),
            Route::Arc(from, control, to) => (0..=CURVE_PIECES)
                .map(|step| {
                    let t = step as f32 / CURVE_PIECES as f32;
                    let rest = 1.0 - t;
                    pos2(
                        rest * rest * from.x + 2.0 * rest * t * control.x + t * t * to.x,
                        rest * rest * from.y + 2.0 * rest * t * control.y + t * t * to.y,
                    )
                })
                .collect(),
        }
    }

    pub(super) fn reversed(self) -> Route {
        match self {
            Route::Line(from, to) => Route::Line(to, from),
            Route::Arc(from, control, to) => Route::Arc(to, control, from),
            Route::Bent([first, second, third, fourth]) => {
                Route::Bent([fourth, third, second, first])
            }
        }
    }

    /// The line an arrowhead at the second end sits on: where the route comes from just before
    /// the end, and the end.
    pub(super) fn arrow_at_end(&self) -> (Pos2, Pos2) {
        match *self {
            Route::Line(from, to) => (from, to),
            Route::Arc(_, control, to) => (control, to),
            Route::Bent(points) => (points[2], points[3]),
        }
    }

    /// The line an arrowhead at the first end sits on: where the route goes just after the start,
    /// and the start.
    pub(super) fn arrow_at_start(&self) -> (Pos2, Pos2) {
        match *self {
            Route::Line(from, to) => (to, from),
            Route::Arc(from, control, _) => (control, from),
            Route::Bent(points) => (points[1], points[0]),
        }
    }
}
