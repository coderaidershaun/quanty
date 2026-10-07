//! Where each panel sits in the window. Pure geometry: it asks nothing of egui but its
//! rectangles.

use eframe::egui::{Rect, pos2, vec2};

use crate::theme::{size, space};

pub const MIN_WINDOW: [f32; 2] = [1180.0, 720.0];
pub const DEFAULT_WINDOW: [f32; 2] = [1536.0, 1024.0];

/// The height of the top bar, in points.
pub const TOP_BAR: f32 = size::CONTROL_LG + 2.0 * space::SM;
/// The height of the Ask bar, in points.
pub const ASK_BAR: f32 = 2.0 * space::MD + size::CONTROL_LG + space::SM + size::CONTROL_SM;

const CLUSTER: [f32; 2] = [132.0, size::CONTROL_LG];
const TABS_WIDTH: f32 = 300.0;

/// The rectangle of every panel of the window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShellRects {
    pub top_bar: Rect,
    pub ask_bar: Rect,
    pub answer: Rect,
    pub source: Rect,
    pub concept_graph: Rect,
    pub retrieval_path: Rect,
    pub follow_up: Rect,
    /// The whole area under the top bar, for Library and Ingest.
    pub page: Rect,
}

/// A plain grid: the Answer and the Source on top, the other three in a row below them.
pub fn shell(window: Rect) -> ShellRects {
    let gutter = space::LG;
    let gap = space::MD;
    let top_bar = Rect::from_min_size(window.min, vec2(window.width(), TOP_BAR));
    let below = top_bar.bottom() + space::XS;
    let left = window.left() + gutter;
    let right = window.right() - gutter;
    let bottom = window.bottom() - gutter;
    let ask_bar = Rect::from_min_size(pos2(left, below), vec2(right - left, ASK_BAR));
    let work_top = ask_bar.bottom() + gap;
    let middle = work_top + (bottom - work_top) * 0.6;
    let split = left + (right - left) * 0.66;
    let third = (right - left - 2.0 * gap) / 3.0;
    let row =
        |from: f32, to: f32, top: f32, low: f32| Rect::from_min_max(pos2(from, top), pos2(to, low));
    let lower = middle + gap;
    ShellRects {
        top_bar,
        ask_bar,
        answer: row(left, split - gap / 2.0, work_top, middle),
        source: row(split + gap / 2.0, right, work_top, middle),
        concept_graph: row(left, left + third, lower, bottom),
        retrieval_path: row(left + third + gap, left + 2.0 * third + gap, lower, bottom),
        follow_up: row(left + 2.0 * third + 2.0 * gap, right, lower, bottom),
        page: Rect::from_min_max(pos2(left, below), pos2(right, bottom)),
    }
}

/// The three places in the top bar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TopBarRects {
    pub logo: Rect,
    pub tabs: Rect,
    /// The health dot, the bell and the help button.
    pub cluster: Rect,
}

pub fn top_bar(bar: Rect) -> TopBarRects {
    let centre = bar.center();
    let gutter = space::LG;
    TopBarRects {
        logo: Rect::from_min_size(
            pos2(bar.left() + gutter, centre.y - size::CONTROL_MD / 2.0),
            vec2(120.0, size::CONTROL_MD),
        ),
        tabs: Rect::from_center_size(centre, vec2(TABS_WIDTH, size::TAB.min(bar.height()))),
        cluster: Rect::from_min_size(
            pos2(
                bar.right() - gutter - CLUSTER[0],
                centre.y - CLUSTER[1] / 2.0,
            ),
            vec2(CLUSTER[0], CLUSTER[1]),
        ),
    }
}
