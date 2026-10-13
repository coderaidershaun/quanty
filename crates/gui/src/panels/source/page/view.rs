//! Where the page picture sits in its viewport, and how zoom and pan move it. It is plain
//! numbers and needs no window.

use eframe::egui::{Pos2, Rect, Vec2, pos2, vec2};

use crate::contract::PageBox;
use crate::theme::space;

/// The well shows this far round a page that fits the width.
const GUTTER: f32 = space::MD;
pub(super) const MIN_ZOOM: f32 = 0.5;
pub(super) const MAX_ZOOM: f32 = 4.0;
/// The stops of the minus and plus buttons. 1.0 is fit to width.
const ZOOM_STOPS: [f32; 13] = [
    0.5, 0.67, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 4.0,
];

/// `zoom` 1.0 fits the page to the width of the viewport's room. `pan` is how far the room's
/// top-left corner is from the page's, in points.
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

/// The room that the page is laid out in, and the picture's own size in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Viewport {
    room: Rect,
    picture: Vec2,
}

impl Viewport {
    /// The room is `area` less a gutter all round, so that the well shows round a page that
    /// fits. An axis too short for both gutters gets none.
    pub(super) fn new(area: Rect, picture: Vec2) -> Self {
        let gutter = |length: f32| if length > 2.0 * GUTTER { GUTTER } else { 0.0 };
        let sides = vec2(gutter(area.width()), gutter(area.height()));
        Viewport {
            room: area.shrink2(sides),
            picture,
        }
    }

    pub(super) fn room(&self) -> Rect {
        self.room
    }

    /// The page's size on screen, in points.
    fn page_size(&self, zoom: f32) -> Vec2 {
        let width = self.room.width() * zoom;
        vec2(width, width * self.picture.y / self.picture.x)
    }

    pub(super) fn clamp(&self, view: View) -> View {
        let zoom = view.zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        let most = (self.page_size(zoom) - self.room.size()).max(Vec2::ZERO);
        View {
            zoom,
            pan: view.pan.clamp(Vec2::ZERO, most),
        }
    }

    pub(super) fn page_rect(&self, view: View) -> Rect {
        let view = self.clamp(view);
        let page = self.page_size(view.zoom);
        Rect::from_min_size(self.room.min + self.centring(page) - view.pan, page)
    }

    /// Changes the zoom and keeps the point of the page under `anchor` where it is.
    pub(super) fn zoom_about(&self, view: View, anchor: Pos2, zoom: f32) -> View {
        let zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        let before = self.page_rect(view);
        let share = (anchor - before.min) / before.size();
        let page = self.page_size(zoom);
        let pan = self.room.min + self.centring(page) + share * page - anchor;
        self.clamp(View { zoom, pan })
    }

    /// Nothing moves when `cut` is in view already. On each axis it is centred when it fits, else
    /// its start is shown.
    pub(super) fn reveal(&self, view: View, cut: PageBox) -> View {
        let view = self.clamp(view);
        let page = self.page_size(view.zoom);
        let target = box_rect(Rect::from_min_size(Pos2::ZERO, page), cut);
        if Rect::from_min_size(view.pan.to_pos2(), self.room.size()).contains_rect(target) {
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
            along(target.left(), target.width(), self.room.width()),
            along(target.top(), target.height(), self.room.height()),
        );
        self.clamp(View {
            zoom: view.zoom,
            pan,
        })
    }

    fn centring(&self, page: Vec2) -> Vec2 {
        ((self.room.size() - page) * 0.5).max(Vec2::ZERO)
    }
}
