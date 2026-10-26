//! Where each panel sits in the window. Pure geometry: it asks nothing of egui but its
//! rectangles.

use eframe::egui::{Rect, Vec2, pos2, vec2};

use crate::theme::{size, space};
use contract::AskView;

/// The smallest window, in points: its width, then its height. Every panel must still work at
/// the size this leaves it.
pub const MIN_WINDOW: [f32; 2] = [1180.0, 720.0];
/// The size the window opens at, in points: its width, then its height.
pub const DEFAULT_WINDOW: [f32; 2] = [1536.0, 1024.0];
/// A tablet held upright, in points.
pub const TABLET_WINDOW: [f32; 2] = [820.0, 1180.0];
/// A phone held upright, in points.
pub const PHONE_WINDOW: [f32; 2] = [390.0, 844.0];
/// A phone held sideways, in points.
pub const PHONE_LANDSCAPE_WINDOW: [f32; 2] = [844.0, 390.0];
/// The smallest window any screen must work in, in points: a small phone held upright.
pub const SMALLEST_WINDOW: [f32; 2] = [360.0, 400.0];
/// The smallest window of the medium form, in points.
const MEDIUM_MIN: [f32; 2] = [720.0, 560.0];

/// How the window is laid out, by its size: the whole desk of panels, the Answer beside one other
/// panel, or one view at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Form {
    Wide,
    Medium,
    Compact,
}

impl Form {
    /// The form of a window of that size, in points. A window as large as `MIN_WINDOW` in both
    /// ways is wide, one as large as the medium minimum in both ways is medium, and any other is
    /// compact.
    pub fn of(size: Vec2) -> Form {
        if size.x >= MIN_WINDOW[0] && size.y >= MIN_WINDOW[1] {
            Form::Wide
        } else if size.x >= MEDIUM_MIN[0] && size.y >= MEDIUM_MIN[1] {
            Form::Medium
        } else {
            Form::Compact
        }
    }
}

/// The height of the top bar, in points.
pub const TOP_BAR: f32 = size::CONTROL_LG + 2.0 * space::SM;
/// The height of the footer, in points.
pub const FOOTER: f32 = size::CONTROL_SM;
/// The height of the Ask bar of a wide window, in points.
pub const ASK_BAR: f32 = 2.0 * space::MD + size::CONTROL_LG + space::SM + size::CONTROL_SM;
/// The height of the Ask bar of a medium window, in points. It may grow to 300 before the panels
/// under it get too small.
pub const ASK_BAR_MEDIUM: f32 = ASK_BAR;
/// The height of the Ask bar of a compact window, in points. The Answer view under it keeps at
/// least 100 points in a phone held sideways only while this stays at 130 or below.
pub const ASK_BAR_COMPACT: f32 = 2.0 * space::SM
    + size::CONTROL_LG
    + space::SM
    + size::CONTROL_MD
    + space::XS
    + size::CONTROL_SM;
/// The height of the bottom navigation of a compact window, in points. Each item fills it.
pub const BOTTOM_NAV: f32 = size::TAB + space::MD;

const TABS_WIDTH: f32 = 300.0;
const LOGO_WIDTH: f32 = 120.0;

const GUTTER: f32 = space::LG;
const GAP: f32 = space::MD;
/// A compact window is narrow, and the sheets of the panels keep their own margin, so it spends
/// less on the space around them.
const COMPACT_GUTTER: f32 = space::SM;
const COMPACT_GAP: f32 = space::SM;
/// The side panel of a medium window takes this share of the room along the longer side, so the
/// Answer always gets the larger share.
const SIDE_SHARE: f32 = 0.45;
/// Beside the Answer, the side panel stops growing at this width.
const SIDE_MAX: f32 = RIGHT_MAX;
/// The height of the strip of tabs over the side panel of a medium window.
const SIDE_TABS: f32 = size::TAB;
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

/// The rectangle of every part the shell can draw, under the same names in every form. Which of
/// them a screen draws depends on the open tab and, on the Ask tab, on `ask_regions`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShellRects {
    pub form: Form,
    pub top_bar: Rect,
    pub ask_bar: Rect,
    /// In a compact window it is the room under the Ask bar.
    pub answer: Rect,
    /// In a medium window the four panels `source`, `concept_graph`, `retrieval_path` and
    /// `follow_up` are all the one side panel. In a compact window they are all the whole room
    /// of a view, with no Ask bar above it.
    pub source: Rect,
    pub concept_graph: Rect,
    pub retrieval_path: Rect,
    pub follow_up: Rect,
    /// The strip of tabs over the side panel of a medium window. `None` in the other forms.
    pub side_tabs: Option<Rect>,
    /// The bottom navigation of a compact window. `None` in the other forms.
    pub bottom_nav: Option<Rect>,
    /// The room under the top bar for every tab but Ask, and for a maximised panel of Ask.
    pub page: Rect,
    /// The whole width of the window, at its foot.
    pub footer: Rect,
}

/// A part of the Ask tab that the shell draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AskRegion {
    AskBar,
    View(AskView),
    SideTabs,
    BottomNav,
}

impl ShellRects {
    /// The parts of the Ask tab that are drawn while no panel is maximised, in drawing order. A
    /// compact window draws only `view`, a medium one draws the Answer beside `side`, and a wide
    /// one draws every view.
    pub fn ask_regions(&self, view: AskView, side: AskView) -> Vec<(AskRegion, Rect)> {
        let regions = match (self.form, view) {
            (Form::Wide, _) => [AskRegion::AskBar]
                .into_iter()
                .chain(AskView::ALL.map(AskRegion::View))
                .collect(),
            (Form::Medium, _) => vec![
                AskRegion::AskBar,
                AskRegion::View(AskView::Answer),
                AskRegion::SideTabs,
                AskRegion::View(side),
            ],
            (Form::Compact, AskView::Answer) => vec![
                AskRegion::AskBar,
                AskRegion::View(AskView::Answer),
                AskRegion::BottomNav,
            ],
            (Form::Compact, _) => vec![AskRegion::View(view), AskRegion::BottomNav],
        };
        regions
            .into_iter()
            .filter_map(|region| Some((region, self.rect_of(region)?)))
            .collect()
    }

    fn rect_of(&self, region: AskRegion) -> Option<Rect> {
        match region {
            AskRegion::AskBar => Some(self.ask_bar),
            AskRegion::View(AskView::Answer) => Some(self.answer),
            AskRegion::View(AskView::Source) => Some(self.source),
            AskRegion::View(AskView::ConceptGraph) => Some(self.concept_graph),
            AskRegion::View(AskView::RetrievalPath) => Some(self.retrieval_path),
            AskRegion::View(AskView::FollowUp) => Some(self.follow_up),
            AskRegion::SideTabs => self.side_tabs,
            AskRegion::BottomNav => self.bottom_nav,
        }
    }
}

/// What every form shares: the top bar and the footer at the edges of the window, and where the
/// room between them starts.
struct Bands {
    top_bar: Rect,
    footer: Rect,
    ask_top: f32,
}

fn bands(window: Rect) -> Bands {
    let footer = edges(
        window.left(),
        window.bottom() - FOOTER,
        window.right(),
        window.bottom(),
    );
    let top_bar = Rect::from_min_size(window.min, vec2(window.width(), TOP_BAR));
    let ask_top = (top_bar.bottom() + space::XS).round();
    Bands {
        top_bar,
        footer,
        ask_top,
    }
}

/// The rectangles of every part of the window, laid out for the form that its size gives.
pub fn shell(window: Rect) -> ShellRects {
    match Form::of(window.size()) {
        Form::Wide => wide(window),
        Form::Medium => medium(window),
        Form::Compact => compact(window),
    }
}

/// Extra height goes to the top row, which is what a person reads.
fn wide(window: Rect) -> ShellRects {
    // Every edge is made whole once, and every other edge is a whole number away from it, so
    // two neighbours never overlap and no border is blurred.
    let left = (window.left() + GUTTER).round();
    let right = (window.right() - GUTTER).round();
    let Bands {
        top_bar,
        footer,
        ask_top,
    } = bands(window);
    let bottom = (footer.top() - space::SM).round();
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
        form: Form::Wide,
        top_bar,
        ask_bar,
        answer: edges(left, work_top, left_right, upper_bottom),
        source: edges(right_left, work_top, right, upper_bottom),
        concept_graph: edges(left, lower_top, graph_right, bottom),
        retrieval_path: edges(graph_right + GAP, lower_top, left_right, bottom),
        follow_up: edges(right_left, lower_top, right, bottom),
        side_tabs: None,
        bottom_nav: None,
        page: edges(left, ask_top, right, bottom),
        footer,
    }
}

/// The Answer and one side panel share the room under the Ask bar along its longer side, so a
/// wide room puts them side by side and a tall room puts the side panel under the Answer.
fn medium(window: Rect) -> ShellRects {
    let left = (window.left() + GUTTER).round();
    let right = (window.right() - GUTTER).round();
    let Bands {
        top_bar,
        footer,
        ask_top,
    } = bands(window);
    let bottom = (footer.top() - space::SM).round();
    let ask_bar = edges(left, ask_top, right, ask_top + ASK_BAR_MEDIUM);

    let work = edges(left, ask_bar.bottom() + GAP, right, bottom);
    let (answer, side_area) = if work.width() > work.height() {
        let side_width = (SIDE_SHARE * (work.width() - GAP)).min(SIDE_MAX).round();
        let side_left = right - side_width;
        (
            edges(left, work.top(), side_left - GAP, bottom),
            edges(side_left, work.top(), right, bottom),
        )
    } else {
        let side_height = (SIDE_SHARE * (work.height() - GAP)).round();
        let side_top = bottom - side_height;
        (
            edges(left, work.top(), right, side_top - GAP),
            edges(left, side_top, right, bottom),
        )
    };
    let side_tabs = edges(
        side_area.left(),
        side_area.top(),
        side_area.right(),
        side_area.top() + SIDE_TABS,
    );
    let side = edges(
        side_area.left(),
        side_tabs.bottom() + space::SM,
        side_area.right(),
        side_area.bottom(),
    );

    ShellRects {
        form: Form::Medium,
        top_bar,
        ask_bar,
        answer,
        source: side,
        concept_graph: side,
        retrieval_path: side,
        follow_up: side,
        side_tabs: Some(side_tabs),
        bottom_nav: None,
        page: edges(left, ask_top, right, bottom),
        footer,
    }
}

/// One view fills the room between the top bar and the bottom navigation. Only the Answer view
/// has the Ask bar above it.
fn compact(window: Rect) -> ShellRects {
    let left = (window.left() + COMPACT_GUTTER).round();
    let right = (window.right() - COMPACT_GUTTER).round();
    let Bands {
        top_bar,
        footer,
        ask_top,
    } = bands(window);
    let bottom_nav = edges(
        window.left(),
        footer.top() - BOTTOM_NAV,
        window.right(),
        footer.top(),
    );
    let view_bottom = (bottom_nav.top() - COMPACT_GAP).round();
    let ask_bar = edges(left, ask_top, right, ask_top + ASK_BAR_COMPACT);
    let view = edges(left, ask_top, right, view_bottom);

    ShellRects {
        form: Form::Compact,
        top_bar,
        ask_bar,
        answer: edges(left, ask_bar.bottom() + COMPACT_GAP, right, view_bottom),
        source: view,
        concept_graph: view,
        retrieval_path: view,
        follow_up: view,
        side_tabs: None,
        bottom_nav: Some(bottom_nav),
        page: edges(left, ask_top, right, (footer.top() - COMPACT_GAP).round()),
        footer,
    }
}

fn edges(left: f32, top: f32, right: f32, bottom: f32) -> Rect {
    Rect::from_min_max(pos2(left, top), pos2(right, bottom))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TopBarRects {
    pub logo: Rect,
    pub tabs: Rect,
}

pub fn top_bar(bar: Rect) -> TopBarRects {
    let centre = bar.center();
    TopBarRects {
        logo: Rect::from_min_size(
            pos2(bar.left() + GUTTER, centre.y - size::CONTROL_MD / 2.0),
            vec2(LOGO_WIDTH, size::CONTROL_MD),
        ),
        tabs: Rect::from_center_size(centre, vec2(TABS_WIDTH, size::TAB.min(bar.height()))),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FooterRects {
    /// The two paid accounts, from the left.
    pub accounts: Rect,
    pub connection: Rect,
}

pub fn footer(footer: Rect) -> FooterRects {
    let row = footer.shrink2(vec2(GUTTER, 0.0));
    let connection = Rect::from_min_max(pos2(row.right() - 240.0, row.top()), row.max);
    let accounts = Rect::from_min_max(row.min, pos2(connection.left() - GUTTER, row.bottom()));
    FooterRects {
        accounts,
        connection,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_form_follows_the_window_size() {
        let cases = [
            ([1180.0, 720.0], Form::Wide),
            ([1179.0, 720.0], Form::Medium),
            ([1180.0, 719.0], Form::Medium),
            ([720.0, 560.0], Form::Medium),
            ([719.0, 560.0], Form::Compact),
            ([720.0, 559.0], Form::Compact),
            (DEFAULT_WINDOW, Form::Wide),
            (MIN_WINDOW, Form::Wide),
            (TABLET_WINDOW, Form::Medium),
            (PHONE_WINDOW, Form::Compact),
            (PHONE_LANDSCAPE_WINDOW, Form::Compact),
            (SMALLEST_WINDOW, Form::Compact),
        ];
        for ([width, height], form) in cases {
            assert_eq!(Form::of(vec2(width, height)), form, "{width} x {height}");
        }
    }
}
