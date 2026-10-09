//! Where each panel sits in the window. Pure geometry: it asks nothing of egui but its
//! rectangles.

use eframe::egui::{Rect, pos2, vec2};

use crate::theme::{size, space};

/// The smallest window, in points: its width, then its height. Every panel must still work at
/// the size this leaves it.
pub const MIN_WINDOW: [f32; 2] = [1180.0, 720.0];
/// The size the window opens at, in points: its width, then its height.
pub const DEFAULT_WINDOW: [f32; 2] = [1536.0, 1024.0];

/// The height of the top bar, in points.
pub const TOP_BAR: f32 = size::CONTROL_LG + 2.0 * space::SM;
/// The height of the Ask bar, in points.
pub const ASK_BAR: f32 = 2.0 * space::MD + size::CONTROL_LG + space::SM + size::CONTROL_SM;

const CLUSTER: [f32; 2] = [132.0, size::CONTROL_LG];
const TABS_WIDTH: f32 = 300.0;
const LOGO_WIDTH: f32 = 120.0;

const GUTTER: f32 = space::LG;
const GAP: f32 = space::MD;
/// The bottom row takes this share of the work area, kept between its two limits.
const BOTTOM_SHARE: f32 = 0.325;
const BOTTOM_MIN: f32 = 220.0;
const BOTTOM_MAX: f32 = 300.0;
/// The right column takes this share of the width, kept between its two limits.
const RIGHT_SHARE: f32 = 0.335;
const RIGHT_MIN: f32 = 420.0;
const RIGHT_MAX: f32 = 620.0;
/// The Concept Graph takes this share of the left column, less one gap.
const GRAPH_SHARE: f32 = 0.57;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShellRects {
    pub top_bar: Rect,
    pub ask_bar: Rect,
    pub answer: Rect,
    pub source: Rect,
    pub concept_graph: Rect,
    pub retrieval_path: Rect,
    pub follow_up: Rect,
    /// The whole area under the top bar, for Library, Ingest and a maximised panel of Ask.
    pub page: Rect,
}

/// Extra height goes to the top row, which is what a person reads.
pub fn shell(window: Rect) -> ShellRects {
    // Every edge is made whole once, and every other edge is a whole number away from it, so
    // two neighbours never overlap and no border is blurred.
    let left = (window.left() + GUTTER).round();
    let right = (window.right() - GUTTER).round();
    let bottom = (window.bottom() - GUTTER).round();
    let top_bar = Rect::from_min_size(window.min, vec2(window.width(), TOP_BAR));
    let ask_top = (top_bar.bottom() + space::XS).round();
    let ask_bar = edges(left, ask_top, right, ask_top + ASK_BAR);

    let work_top = ask_bar.bottom() + GAP;
    let bottom_height = (BOTTOM_SHARE * (bottom - work_top))
        .clamp(BOTTOM_MIN, BOTTOM_MAX)
        .round();
    let lower_top = bottom - bottom_height;
    let upper_bottom = lower_top - GAP;

    let right_width = (RIGHT_SHARE * (right - left))
        .clamp(RIGHT_MIN, RIGHT_MAX)
        .round();
    let right_left = right - right_width;
    let left_right = right_left - GAP;
    let graph_width = (GRAPH_SHARE * (left_right - left - GAP)).round();
    let graph_right = left + graph_width;

    ShellRects {
        top_bar,
        ask_bar,
        answer: edges(left, work_top, left_right, upper_bottom),
        source: edges(right_left, work_top, right, upper_bottom),
        concept_graph: edges(left, lower_top, graph_right, bottom),
        retrieval_path: edges(graph_right + GAP, lower_top, left_right, bottom),
        follow_up: edges(right_left, lower_top, right, bottom),
        page: edges(left, ask_top, right, bottom),
    }
}

fn edges(left: f32, top: f32, right: f32, bottom: f32) -> Rect {
    Rect::from_min_max(pos2(left, top), pos2(right, bottom))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TopBarRects {
    pub logo: Rect,
    pub tabs: Rect,
    /// The health dot, the bell and the help button.
    pub cluster: Rect,
}

pub fn top_bar(bar: Rect) -> TopBarRects {
    let centre = bar.center();
    TopBarRects {
        logo: Rect::from_min_size(
            pos2(bar.left() + GUTTER, centre.y - size::CONTROL_MD / 2.0),
            vec2(LOGO_WIDTH, size::CONTROL_MD),
        ),
        tabs: Rect::from_center_size(centre, vec2(TABS_WIDTH, size::TAB.min(bar.height()))),
        cluster: Rect::from_min_size(
            pos2(
                bar.right() - GUTTER - CLUSTER[0],
                centre.y - CLUSTER[1] / 2.0,
            ),
            vec2(CLUSTER[0], CLUSTER[1]),
        ),
    }
}
