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

/// Where the canvas looks at the picture, and how large the picture is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(super) struct View {
    /// The point of the picture that is at the centre of the canvas.
    pub at: Point,
    /// Doublings of the size. The drawn size snaps to quarter steps, so that a long pinch asks
    /// the font for only nine sizes of each text.
    pub zoom_log: f32,
}

impl View {
    pub(super) fn step(&self) -> i8 {
        (self.zoom_log * STEPS_PER_DOUBLING).round() as i8
    }

    pub(super) fn scale(&self) -> f32 {
        scale_of(self.step())
    }

    /// The view a new picture opens in: all of it when that is readable, else full size on
    /// `centre`.
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
            None => View {
                at: centre,
                zoom_log: 0.0,
            }
            .held(world, canvas),
        }
    }

    /// The canvas point of a point of the picture.
    pub(super) fn point_on_canvas(&self, canvas_centre: Point, at: Point) -> Point {
        let scale = self.scale();
        point(
            canvas_centre.x + (at.x - self.at.x) * scale,
            canvas_centre.y + (at.y - self.at.y) * scale,
        )
    }

    /// The canvas rectangle of a rectangle of the picture.
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

    /// Zooms by `doublings`. The point of the picture under `pointer` (measured from the canvas
    /// centre) stays where it is.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn world(width: f32, height: f32) -> Rect {
        Rect::from_min_size(
            point(-width / 2.0, 0.0),
            Size {
                w: width,
                h: height,
            },
        )
    }

    /// The part of the picture that the canvas shows.
    fn seen(view: View, canvas: Size) -> Rect {
        Rect::from_centre(
            view.at,
            Size {
                w: canvas.w / view.scale(),
                h: canvas.h / view.scale(),
            },
        )
    }

    /// Along each side the canvas is inside the picture, or the picture is centred on it.
    fn nothing_is_lost(view: View, world: Rect, canvas: Size) -> bool {
        let seen = seen(view, canvas);
        let along = |low: f32, high: f32, seen_low: f32, seen_high: f32| {
            let centred = ((seen_low + seen_high) - (low + high)).abs() < 0.01;
            let inside = seen_low >= low - 0.01 && seen_high <= high + 0.01;
            if seen_high - seen_low >= high - low {
                centred
            } else {
                inside
            }
        };
        along(world.min.x, world.max.x, seen.min.x, seen.max.x)
            && along(world.min.y, world.max.y, seen.min.y, seen.max.y)
    }

    #[test]
    fn a_view_opens_fitted_or_on_the_centre_and_never_loses_the_picture() {
        let canvas = Size { w: 522.0, h: 179.0 };

        // A picture that fits opens whole, at the largest step that shows it all.
        let small = world(400.0, 150.0);
        let opened = View::opening(small, canvas, point(0.0, 75.0));
        assert_eq!((opened.step(), opened.at), (0, small.centre()));
        let wide = world(600.0, 150.0);
        let opened = View::opening(wide, canvas, point(0.0, 75.0));
        assert_eq!((opened.step(), opened.at), (-1, wide.centre()));

        // One that needs more than 71 % opens at 100 % on the centre node, kept over the picture.
        let large = world(800.0, 150.0);
        let opened = View::opening(large, canvas, point(-300.0, 75.0));
        assert_eq!(
            opened,
            View {
                at: point(-139.0, 75.0),
                zoom_log: 0.0
            }
        );

        // A pan, a zoom or a smaller window cannot lose the picture, however far it goes.
        let big = world(3000.0, 400.0);
        let start = View::opening(big, canvas, point(0.0, 200.0));
        for by in [
            point(5000.0, 0.0),
            point(-5000.0, 900.0),
            point(30.0, -4000.0),
        ] {
            assert!(nothing_is_lost(
                start.panned(by).held(big, canvas),
                big,
                canvas
            ));
        }
        let tiny = Size { w: 300.0, h: 120.0 };
        let zoomed_out = start.zoomed(-9.0, point(0.0, 0.0)).held(big, tiny);
        assert!(nothing_is_lost(zoomed_out, big, tiny));
        let flat = world(3000.0, 100.0);
        assert!(nothing_is_lost(start.held(flat, canvas), flat, canvas));

        // The zoom stops at 50 % and 200 %, and snaps to quarter steps.
        assert_eq!(start.zoomed(-9.0, point(0.0, 0.0)).step(), -4);
        assert_eq!(start.zoomed(9.0, point(0.0, 0.0)).step(), 4);
        assert_eq!(
            View {
                zoom_log: 0.1,
                ..start
            }
            .step(),
            0
        );
        assert_eq!(
            View {
                zoom_log: 0.13,
                ..start
            }
            .step(),
            1
        );

        // A zoom keeps the point under the pointer where it is.
        let pointer = point(100.0, 20.0);
        let under = |view: View| {
            point(
                view.at.x + pointer.x / view.scale(),
                view.at.y + pointer.y / view.scale(),
            )
        };
        let after = start.zoomed(0.5, pointer);
        assert_eq!(after.step(), 2);
        assert!((under(after).x - under(start).x).abs() < 0.001);
        assert!((under(after).y - under(start).y).abs() < 0.001);

        // Showing a rectangle moves the view as little as it must, and not at all when the
        // rectangle is in sight.
        let far = Rect::from_min_size(point(900.0, 180.0), Size { w: 100.0, h: 40.0 });
        let moved = start.showing(far, canvas);
        assert_eq!(moved.at, point(1000.0 - canvas.w / 2.0, start.at.y));
        let near = Rect::from_min_size(point(-20.0, 190.0), Size { w: 40.0, h: 20.0 });
        assert_eq!(start.showing(near, canvas), start);
    }
}
