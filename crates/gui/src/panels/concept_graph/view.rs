//! Pan and zoom as plain numbers: which point of the picture is at the centre of the canvas, at
//! what size, and the limits of both.

use super::geometry::{Point, Rect, Size, point};

/// The smallest and the largest step of the zoom: 50 % and 200 %.
const MIN_STEP: i8 = -4;
const MAX_STEP: i8 = 4;
/// A new picture opens showing everything when that needs no less than this step: 71 %.
const FIT_FLOOR: i8 = -2;
const STEPS_PER_DOUBLING: f32 = 4.0;
const MIN_ZOOM: f32 = MIN_STEP as f32 / STEPS_PER_DOUBLING;
const MAX_ZOOM: f32 = MAX_STEP as f32 / STEPS_PER_DOUBLING;
/// A picture that overshoots the canvas by less than half a point still counts as fitting.
const FIT_SLACK: f32 = 0.5;

fn scale_of(step: i8) -> f32 {
    2.0_f32.powf(f32::from(step) / STEPS_PER_DOUBLING)
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(super) struct View {
    /// The point of the picture that is at the centre of the canvas.
    pub at: Point,
    /// The zoom is counted in doublings of the size. The drawn size snaps to quarter steps, so
    /// that a long pinch asks the font for only nine sizes of each text.
    pub zoom_log: f32,
}

impl View {
    pub(super) fn step(&self) -> i8 {
        (self.zoom_log * STEPS_PER_DOUBLING).round() as i8
    }

    pub(super) fn scale(&self) -> f32 {
        scale_of(self.step())
    }

    /// The view a new picture opens in: all of it when that is readable. Else the smallest
    /// readable size, on the middle of the picture when `centre` (the main concept) is then in
    /// sight, so that the cut is shared by both sides, and on `centre` when it is not.
    pub(super) fn opening(world: Rect, canvas: Size, centre: Point) -> View {
        let fits = |step: i8| {
            let scale = scale_of(step);
            world.width() * scale <= canvas.w + FIT_SLACK
                && world.height() * scale <= canvas.h + FIT_SLACK
        };
        match (FIT_FLOOR..=0).rev().find(|step| fits(*step)) {
            Some(step) => View {
                at: world.centre(),
                zoom_log: f32::from(step) / STEPS_PER_DOUBLING,
            },
            None => {
                let (half_w, half_h) = (
                    canvas.w / scale_of(FIT_FLOOR) / 2.0,
                    canvas.h / scale_of(FIT_FLOOR) / 2.0,
                );
                let middle = world.centre();
                let centre_is_in_sight =
                    (centre.x - middle.x).abs() <= half_w && (centre.y - middle.y).abs() <= half_h;
                View {
                    at: if centre_is_in_sight { middle } else { centre },
                    zoom_log: f32::from(FIT_FLOOR) / STEPS_PER_DOUBLING,
                }
                .held(world, canvas)
            }
        }
    }

    pub(super) fn point_on_canvas(&self, canvas_centre: Point, at: Point) -> Point {
        let scale = self.scale();
        point(
            canvas_centre.x + (at.x - self.at.x) * scale,
            canvas_centre.y + (at.y - self.at.y) * scale,
        )
    }

    pub(super) fn rect_on_canvas(&self, canvas_centre: Point, rect: Rect) -> Rect {
        Rect {
            min: self.point_on_canvas(canvas_centre, rect.min),
            max: self.point_on_canvas(canvas_centre, rect.max),
        }
    }

    /// Keeps the canvas over the picture. Along a side that the picture does not fill, the
    /// picture is centred.
    pub(super) fn held(mut self, world: Rect, canvas: Size) -> View {
        self.zoom_log = self.zoom_log.clamp(MIN_ZOOM, MAX_ZOOM);
        let scale = self.scale();
        self.at = point(
            hold(self.at.x, (world.min.x, world.max.x), canvas.w / scale),
            hold(self.at.y, (world.min.y, world.max.y), canvas.h / scale),
        );
        self
    }

    /// A drag by `by` points of the screen.
    pub(super) fn panned(mut self, by: Point) -> View {
        let scale = self.scale();
        self.at = point(self.at.x - by.x / scale, self.at.y - by.y / scale);
        self
    }

    /// The point of the picture under `pointer` (measured from the canvas centre) stays where
    /// it is.
    pub(super) fn zoomed(mut self, doublings: f32, pointer: Point) -> View {
        let before = self.scale();
        let under = point(
            self.at.x + pointer.x / before,
            self.at.y + pointer.y / before,
        );
        self.zoom_log = (self.zoom_log + doublings).clamp(MIN_ZOOM, MAX_ZOOM);
        let after = self.scale();
        self.at = point(under.x - pointer.x / after, under.y - pointer.y / after);
        self
    }

    /// Moves as little as needed to bring `rect` of the picture onto the canvas.
    pub(super) fn showing(mut self, rect: Rect, canvas: Size) -> View {
        let scale = self.scale();
        let (half_w, half_h) = (canvas.w / scale / 2.0, canvas.h / scale / 2.0);
        self.at = point(
            show(self.at.x, (rect.min.x, rect.max.x), half_w),
            show(self.at.y, (rect.min.y, rect.max.y), half_h),
        );
        self
    }
}

/// Along one side: the picture `(low, high)` centred when the canvas is wider than it, else
/// `at` kept so that the canvas stays inside it.
fn hold(at: f32, (low, high): (f32, f32), seen: f32) -> f32 {
    if high - low <= seen {
        (low + high) / 2.0
    } else {
        at.clamp(low + seen / 2.0, high - seen / 2.0)
    }
}

/// Along one side: `at` moved the least that puts `(low, high)` in sight, or centred on it
/// when it is larger than the canvas, whose half size is `half`.
fn show(at: f32, (low, high): (f32, f32), half: f32) -> f32 {
    if high - low > 2.0 * half {
        (low + high) / 2.0
    } else {
        at.clamp(high - half, low + half)
    }
}
