//! Where the page picture sits in its viewport, and how zoom and pan move it. It is plain
//! numbers, so it needs no window to test.

use eframe::egui::{Pos2, Rect, Vec2, pos2, vec2};

use crate::contract::PageBox;
use crate::theme::space;

pub(super) const MIN_ZOOM: f32 = 0.5;
pub(super) const MAX_ZOOM: f32 = 4.0;
/// The stops of the minus and plus buttons. 1.0 is fit to width.
const ZOOM_STOPS: [f32; 13] = [
    0.5, 0.67, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 4.0,
];

/// `zoom` 1.0 fits the page to the viewport's width. `pan` is how far the viewport's top-left
/// corner is from the page's, in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct View {
    pub zoom: f32,
    pub pan: Vec2,
}

impl Default for View {
    fn default() -> Self {
        View {
            zoom: 1.0,
            pan: Vec2::ZERO,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ZoomStep {
    In,
    Out,
}

/// The next stop above or below `zoom`.
pub(super) fn step_zoom(zoom: f32, step: ZoomStep) -> f32 {
    const SAME: f32 = 0.005;
    let next = match step {
        ZoomStep::In => ZOOM_STOPS.iter().copied().find(|stop| *stop > zoom + SAME),
        ZoomStep::Out => ZOOM_STOPS
            .iter()
            .rev()
            .copied()
            .find(|stop| *stop < zoom - SAME),
    };
    next.unwrap_or(zoom.clamp(MIN_ZOOM, MAX_ZOOM))
}

/// A rectangle given in thousandths of the page, on screen.
pub(super) fn box_rect(page: Rect, cut: PageBox) -> Rect {
    let at = |x: u16, y: u16| {
        pos2(
            page.left() + page.width() * f32::from(x.min(1000)) / 1000.0,
            page.top() + page.height() * f32::from(y.min(1000)) / 1000.0,
        )
    };
    Rect::from_two_pos(at(cut.left, cut.top), at(cut.right, cut.bottom))
}

/// The rectangle of the screen the page is shown in, and the picture's own size in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Viewport {
    pub rect: Rect,
    pub picture: Vec2,
}

impl Viewport {
    /// The page's size on screen, in points.
    fn page_size(&self, zoom: f32) -> Vec2 {
        let width = self.rect.width() * zoom;
        vec2(width, width * self.picture.y / self.picture.x)
    }

    /// The zoom inside its range, and the pan such that the page cannot leave the viewport.
    pub(super) fn clamp(&self, view: View) -> View {
        let zoom = view.zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        let most = (self.page_size(zoom) - self.rect.size()).max(Vec2::ZERO);
        View {
            zoom,
            pan: view.pan.clamp(Vec2::ZERO, most),
        }
    }

    /// Where the page is drawn. On an axis where it is smaller than the viewport it is centred.
    pub(super) fn page_rect(&self, view: View) -> Rect {
        let view = self.clamp(view);
        let page = self.page_size(view.zoom);
        Rect::from_min_size(self.rect.min + self.centring(page) - view.pan, page)
    }

    /// Changes the zoom and keeps the point of the page under `anchor` where it is.
    pub(super) fn zoom_about(&self, view: View, anchor: Pos2, zoom: f32) -> View {
        let zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        let before = self.page_rect(view);
        let share = (anchor - before.min) / before.size();
        let page = self.page_size(zoom);
        let pan = self.rect.min + self.centring(page) + share * page - anchor;
        self.clamp(View { zoom, pan })
    }

    /// The view that shows `cut`. Nothing moves when it is in view already. On each axis it is
    /// centred when it fits, else its start is shown.
    pub(super) fn reveal(&self, view: View, cut: PageBox) -> View {
        let view = self.clamp(view);
        let page = self.page_size(view.zoom);
        let target = box_rect(Rect::from_min_size(Pos2::ZERO, page), cut);
        if Rect::from_min_size(view.pan.to_pos2(), self.rect.size()).contains_rect(target) {
            return view;
        }
        let along = |from: f32, length: f32, room: f32| {
            if length + 2.0 * space::MD <= room {
                from - (room - length) * 0.5
            } else {
                from - space::MD
            }
        };
        let pan = vec2(
            along(target.left(), target.width(), self.rect.width()),
            along(target.top(), target.height(), self.rect.height()),
        );
        self.clamp(View {
            zoom: view.zoom,
            pan,
        })
    }

    fn centring(&self, page: Vec2) -> Vec2 {
        ((self.rect.size() - page) * 0.5).max(Vec2::ZERO)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEWPORT: Viewport = Viewport {
        rect: Rect::from_min_max(pos2(16.0, 168.0), pos2(488.0, 495.0)),
        picture: vec2(712.0, 1167.0),
    };
    const FIGURE: PageBox = PageBox {
        left: 75,
        top: 510,
        right: 970,
        bottom: 960,
    };

    #[test]
    fn the_page_fits_the_width_zooms_about_a_point_and_cannot_be_lost() {
        let page = VIEWPORT.page_size(1.0);
        assert_eq!(page.x, 472.0);
        assert!((page.y - 773.6).abs() < 0.1, "{page:?}");

        let lost = VIEWPORT.clamp(View {
            zoom: 9.0,
            pan: vec2(-50.0, 99_999.0),
        });
        assert_eq!(lost.zoom, MAX_ZOOM);
        assert_eq!(lost.pan, vec2(0.0, VIEWPORT.page_size(MAX_ZOOM).y - 327.0));
        let small = View {
            zoom: 0.5,
            pan: vec2(30.0, 30.0),
        };
        assert_eq!(VIEWPORT.clamp(small).pan.x, 0.0);
        assert_eq!(
            VIEWPORT.page_rect(small).center().x,
            VIEWPORT.rect.center().x
        );

        let view = View {
            zoom: 1.0,
            pan: vec2(0.0, 200.0),
        };
        let anchor = pos2(300.0, 400.0);
        let before = VIEWPORT.page_rect(view);
        let share = (anchor - before.min) / before.size();
        let zoomed = VIEWPORT.zoom_about(view, anchor, 2.0);
        let after = VIEWPORT.page_rect(zoomed);
        assert!(((after.min + share * after.size()) - anchor).length() < 0.01);

        assert_eq!(step_zoom(1.0, ZoomStep::In), 1.1);
        assert_eq!(step_zoom(1.0, ZoomStep::Out), 0.9);
        assert_eq!(step_zoom(1.3, ZoomStep::In), 1.5);
        assert_eq!(step_zoom(4.0, ZoomStep::In), 4.0);
        assert_eq!(step_zoom(0.5, ZoomStep::Out), 0.5);
    }

    #[test]
    fn a_figure_rectangle_lands_on_the_screen_and_is_brought_into_view() {
        let page = Rect::from_min_size(pos2(16.0, -100.0), vec2(472.0, 773.6));
        let on_screen = box_rect(page, FIGURE);
        assert!(
            (on_screen.left() - 51.4).abs() < 0.01 && (on_screen.right() - 473.84).abs() < 0.01
        );
        assert!(
            (on_screen.top() - 294.536).abs() < 0.01 && (on_screen.bottom() - 642.656).abs() < 0.01
        );

        // The figure is 348 high and the viewport 327: its top is shown, with the margin.
        let shown = VIEWPORT.reveal(View::default(), FIGURE);
        let top = 0.510 * VIEWPORT.page_size(1.0).y;
        assert!((shown.pan.y - (top - space::MD)).abs() < 0.01, "{shown:?}");
        // A small rectangle near the top is in view already: nothing moves.
        let small = PageBox {
            left: 100,
            top: 20,
            right: 300,
            bottom: 90,
        };
        assert_eq!(VIEWPORT.reveal(View::default(), small), View::default());
        // A small rectangle further down is centred.
        let low = PageBox {
            left: 100,
            top: 600,
            right: 300,
            bottom: 700,
        };
        let centred = VIEWPORT.reveal(View::default(), low);
        let middle = 0.65 * VIEWPORT.page_size(1.0).y;
        assert!(
            (centred.pan.y + 327.0 / 2.0 - middle).abs() < 0.01,
            "{centred:?}"
        );
        // One at the foot of the page stops where the page ends.
        let foot = PageBox {
            left: 100,
            top: 900,
            right: 300,
            bottom: 990,
        };
        let page = VIEWPORT.page_size(1.0);
        assert_eq!(VIEWPORT.reveal(View::default(), foot).pan.y, page.y - 327.0);
    }
}
