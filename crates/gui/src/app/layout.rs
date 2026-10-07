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

/// The shell as the wireframe draws it: the Answer and the Source on top, the Concept Graph, the
/// Retrieval Path and the Follow up below. The proportions are fixed, so the shell is a plain
/// function of the window. Extra height goes to the top row, which is what a person reads.
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

/// The three places in the top bar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TopBarRects {
    pub logo: Rect,
    pub tabs: Rect,
    /// The health dot, the bell and the help button.
    pub cluster: Rect,
}

/// Where the logo, the tabs and the group at the right sit in the top bar.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn window(width: f32, height: f32) -> Rect {
        Rect::from_min_size(pos2(0.0, 0.0), vec2(width, height))
    }

    fn panels(rects: &ShellRects) -> [(&'static str, Rect); 6] {
        [
            ("ask bar", rects.ask_bar),
            ("answer", rects.answer),
            ("source", rects.source),
            ("concept graph", rects.concept_graph),
            ("retrieval path", rects.retrieval_path),
            ("follow up", rects.follow_up),
        ]
    }

    fn size_of(rect: Rect) -> [f32; 2] {
        [rect.width(), rect.height()]
    }

    /// Each side is at least the wanted one and at most a point more.
    fn is_wanted_size(rect: Rect, wanted: [f32; 2]) -> bool {
        let [width, height] = size_of(rect);
        (wanted[0]..=wanted[0] + 1.0).contains(&width)
            && (wanted[1]..=wanted[1] + 1.0).contains(&height)
    }

    fn assert_every_window_fits() {
        let mut widths = vec![MIN_WINDOW[0], MIN_WINDOW[0] + 0.5, 1536.0, 3840.0];
        widths.extend(
            (0..)
                .map(|step| 1181.0 + 97.0 * step as f32)
                .take_while(|w| *w < 3840.0),
        );
        let mut heights = vec![MIN_WINDOW[1], MIN_WINDOW[1] + 0.5, 1024.0, 2160.0];
        heights.extend(
            (0..)
                .map(|step| 721.0 + 83.0 * step as f32)
                .take_while(|h| *h < 2160.0),
        );
        for width in &widths {
            for height in &heights {
                let bounds = window(*width, *height);
                let rects = shell(bounds);
                let mut every = panels(&rects).to_vec();
                every.push(("top bar", rects.top_bar));
                every.push(("page", rects.page));
                for (name, rect) in &every {
                    assert!(
                        bounds.contains_rect(*rect),
                        "{name} leaves a {width} x {height} window: {rect:?}"
                    );
                    assert!(rect.is_positive(), "{name} is empty at {width} x {height}");
                }
                for (name, rect) in panels(&rects) {
                    let edges = [rect.left(), rect.top(), rect.right(), rect.bottom()];
                    assert!(
                        edges.iter().all(|edge| edge.fract() == 0.0),
                        "{name} has a blurred edge at {width} x {height}: {rect:?}"
                    );
                }
                let named = panels(&rects);
                for (first, (name, rect)) in named.iter().enumerate() {
                    for (other_name, other) in &named[first + 1..] {
                        assert!(
                            !rect.intersects(*other),
                            "{name} and {other_name} overlap at {width} x {height}"
                        );
                    }
                    assert!(
                        !rect.intersects(rects.top_bar),
                        "{name} overlaps the top bar at {width} x {height}"
                    );
                }
                assert!(!rects.page.intersects(rects.top_bar));
            }
        }
    }

    fn assert_smallest_window_sizes() {
        let rects = shell(window(MIN_WINDOW[0], MIN_WINDOW[1]));
        let wanted = [
            ("ask bar", rects.ask_bar, [1148.0, 100.0]),
            ("answer", rects.answer, [716.0, 300.0]),
            ("source", rects.source, [420.0, 300.0]),
            ("concept graph", rects.concept_graph, [401.0, 220.0]),
            ("retrieval path", rects.retrieval_path, [303.0, 220.0]),
            ("follow up", rects.follow_up, [420.0, 220.0]),
        ];
        for (name, rect, size) in wanted {
            assert!(
                is_wanted_size(rect, size),
                "{name} is {:?}, wanted {size:?}",
                size_of(rect)
            );
        }
    }

    fn assert_default_window_matches_the_wireframe() {
        let rects = shell(window(DEFAULT_WINDOW[0], DEFAULT_WINDOW[1]));
        let measured = [
            ("top bar", rects.top_bar, [0.0, 0.0, 1536.0, 60.0]),
            ("ask bar", rects.ask_bar, [18.0, 64.0, 1500.0, 104.0]),
            ("answer", rects.answer, [18.0, 178.0, 986.0, 549.0]),
            ("source", rects.source, [1015.0, 178.0, 503.0, 549.0]),
            (
                "concept graph",
                rects.concept_graph,
                [18.0, 738.0, 558.0, 271.0],
            ),
            (
                "retrieval path",
                rects.retrieval_path,
                [587.0, 738.0, 421.0, 271.0],
            ),
            ("follow up", rects.follow_up, [1015.0, 738.0, 503.0, 271.0]),
        ];
        for (name, rect, [x, y, width, height]) in measured {
            let found = [rect.left(), rect.top(), rect.width(), rect.height()];
            for (found, wanted) in found.iter().zip([x, y, width, height]) {
                assert!(
                    (found - wanted).abs() <= 8.0,
                    "{name} is at {found:?}, the wireframe has {wanted}"
                );
            }
        }
    }

    #[test]
    fn the_shell_fits_every_window_from_the_smallest_up() {
        assert_every_window_fits();
        assert_smallest_window_sizes();
        assert_default_window_matches_the_wireframe();
    }
}
