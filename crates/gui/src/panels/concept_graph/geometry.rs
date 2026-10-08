//! Plain points, sizes, rectangles and link routes, so that the layout can be worked out without
//! a window.

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(super) struct Point {
    pub x: f32,
    pub y: f32,
}

pub(super) const fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(super) struct Size {
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(super) struct Rect {
    pub min: Point,
    pub max: Point,
}

impl Rect {
    pub(super) fn from_min_size(min: Point, size: Size) -> Rect {
        Rect {
            min,
            max: point(min.x + size.w, min.y + size.h),
        }
    }

    pub(super) fn from_centre(centre: Point, size: Size) -> Rect {
        Rect::from_min_size(
            point(centre.x - size.w / 2.0, centre.y - size.h / 2.0),
            size,
        )
    }

    pub(super) fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    pub(super) fn height(&self) -> f32 {
        self.max.y - self.min.y
    }

    pub(super) fn size(&self) -> Size {
        Size {
            w: self.width(),
            h: self.height(),
        }
    }

    pub(super) fn centre(&self) -> Point {
        point(
            (self.min.x + self.max.x) / 2.0,
            (self.min.y + self.max.y) / 2.0,
        )
    }

    pub(super) fn inflate(&self, by: f32) -> Rect {
        Rect {
            min: point(self.min.x - by, self.min.y - by),
            max: point(self.max.x + by, self.max.y + by),
        }
    }

    pub(super) fn overlaps(&self, other: &Rect) -> bool {
        self.min.x < other.max.x
            && other.min.x < self.max.x
            && self.min.y < other.max.y
            && other.min.y < self.max.y
    }

    pub(super) fn holds(&self, other: &Rect) -> bool {
        self.min.x <= other.min.x
            && self.min.y <= other.min.y
            && self.max.x >= other.max.x
            && self.max.y >= other.max.y
    }

    /// True when the segment from `from` to `to` passes through the inside of the rectangle.
    /// Clips the segment to each of the four sides in turn.
    pub(super) fn cut_by(&self, from: Point, to: Point) -> bool {
        let (dx, dy) = (to.x - from.x, to.y - from.y);
        let (mut enters, mut leaves) = (0.0_f32, 1.0_f32);
        let sides = [
            (-dx, from.x - self.min.x),
            (dx, self.max.x - from.x),
            (-dy, from.y - self.min.y),
            (dy, self.max.y - from.y),
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
}

const CURVE_PIECES: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Route {
    Line(Point, Point),
    /// The three points are the start, the control and the end of a quadratic curve.
    Arc(Point, Point, Point),
    /// The route goes down a band, through a gap of the middle row, and down the next band.
    Bent([Point; 4]),
}

impl Route {
    /// The route as straight pieces: the points between them, in order.
    pub(super) fn pieces(&self) -> Vec<Point> {
        match *self {
            Route::Line(from, to) => vec![from, to],
            Route::Bent(points) => points.to_vec(),
            Route::Arc(from, control, to) => (0..=CURVE_PIECES)
                .map(|step| {
                    let t = step as f32 / CURVE_PIECES as f32;
                    let rest = 1.0 - t;
                    point(
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
    pub(super) fn arrow_at_end(&self) -> (Point, Point) {
        match *self {
            Route::Line(from, to) => (from, to),
            Route::Arc(_, control, to) => (control, to),
            Route::Bent(points) => (points[2], points[3]),
        }
    }

    /// The line an arrowhead at the first end sits on: where the route goes just after the start,
    /// and the start.
    pub(super) fn arrow_at_start(&self) -> (Point, Point) {
        match *self {
            Route::Line(from, to) => (to, from),
            Route::Arc(from, control, _) => (control, from),
            Route::Bent(points) => (points[1], points[0]),
        }
    }
}
