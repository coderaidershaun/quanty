//! Checks that every window from a small phone to a large screen gets a shell whose drawn parts
//! all lie inside the window, never overlap, and are never smaller than a panel can work in.
//!
//! This is a fitness test because the failure is silent: a part that leaves the window, sits on
//! another, or shrinks below what its controls need is still drawn with no error, and the sizes
//! of the windows that people open the app in are not a space that a list of examples can cover.

use contract::AskView;
use eframe::egui::{Pos2, Rect, Vec2, vec2};
use gui::app::layout::{
    self, AskRegion, DEFAULT_WINDOW, Form, MIN_WINDOW, NoteRow, PHONE_LANDSCAPE_WINDOW,
    PHONE_WINDOW, SMALLEST_WINDOW, TABLET_WINDOW,
};
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};

/// The smallest rectangle a panel can draw its controls in without cutting any off, in points.
const PANEL_MIN: Vec2 = vec2(300.0, 100.0);
/// The smallest height of a control that is pressed with a thumb, in points.
const THUMB_MIN: f32 = 44.0;

/// The windows where the form changes, the windows the tests use, and the corners of the range.
const NAMED_WINDOWS: [[f32; 2]; 13] = [
    DEFAULT_WINDOW,
    MIN_WINDOW,
    TABLET_WINDOW,
    PHONE_WINDOW,
    PHONE_LANDSCAPE_WINDOW,
    SMALLEST_WINDOW,
    [1179.0, 720.0],
    [1180.0, 719.0],
    [720.0, 560.0],
    [719.0, 560.0],
    [720.0, 559.0],
    [360.0, 1600.0],
    [2560.0, 1600.0],
];

#[derive(Debug, Clone, Copy, PartialEq)]
enum Part {
    TopBar,
    Footer,
    /// The page of Library, Ingest and Datasets, and of a maximised panel.
    Page,
    Ask(AskRegion),
}

/// What the shell draws on a screen, as the rectangle of each part.
fn drawn(rects: &layout::ShellRects, ask: Option<(AskView, AskView)>) -> Vec<(Part, Rect)> {
    let mut parts = vec![(Part::TopBar, rects.top_bar), (Part::Footer, rects.footer)];
    match ask {
        None => parts.push((Part::Page, rects.page)),
        Some((view, side)) => parts.extend(
            rects
                .ask_regions(view, side)
                .into_iter()
                .map(|(region, rect)| (Part::Ask(region), rect)),
        ),
    }
    parts
}

/// The parts that a form must draw on the Ask tab, as a person sees it.
fn required(form: Form, view: AskView, side: AskView) -> Vec<AskRegion> {
    match form {
        Form::Wide => std::iter::once(AskRegion::AskBar)
            .chain(AskView::ALL.into_iter().map(AskRegion::View))
            .collect(),
        Form::Medium => vec![
            AskRegion::AskBar,
            AskRegion::View(AskView::Answer),
            AskRegion::SideTabs,
            AskRegion::View(side),
        ],
        Form::Compact => {
            let mut parts = vec![AskRegion::View(view), AskRegion::BottomNav];
            if view == AskView::Answer {
                parts.push(AskRegion::AskBar);
            }
            parts
        }
    }
}

fn is_a_panel(part: Part) -> bool {
    matches!(part, Part::Page | Part::Ask(AskRegion::View(_)))
}

fn problems_of(
    window: Rect,
    form: Form,
    ask: Option<(AskView, AskView)>,
    parts: &[(Part, Rect)],
) -> Vec<String> {
    let mut found = Vec::new();
    let mut report = |problem: String| found.push(format!("{ask:?}: {problem}"));
    for (index, (part, rect)) in parts.iter().enumerate() {
        if !rect.is_finite() || !window.contains_rect(*rect) {
            report(format!("{part:?} {rect:?} is not inside the window"));
        }
        for (other, other_rect) in &parts[index + 1..] {
            let overlap = rect.intersect(*other_rect);
            if overlap.width() > 0.0 && overlap.height() > 0.0 {
                report(format!(
                    "{part:?} {rect:?} overlaps {other:?} {other_rect:?}"
                ));
            }
        }
        let size = rect.size();
        if is_a_panel(*part) && (size.x < PANEL_MIN.x || size.y < PANEL_MIN.y) {
            report(format!("{part:?} is only {size:?}"));
        }
        if *part == Part::Ask(AskRegion::AskBar) && size.x < PANEL_MIN.x {
            report(format!("the Ask bar is only {} wide", size.x));
        }
        if *part == Part::Ask(AskRegion::BottomNav) && size.y < THUMB_MIN {
            report(format!("the bottom navigation is only {} tall", size.y));
        }
    }
    if let Some((view, side)) = ask {
        for region in required(form, view, side) {
            if !parts.iter().any(|(part, _)| *part == Part::Ask(region)) {
                report(format!("{region:?} is not drawn"));
            }
        }
    }
    found
}

/// Every way the shell can be drawn in a window of that size, with a note on the Ask bar and
/// without one.
fn problems(size: Vec2) -> Vec<String> {
    let window = Rect::from_min_size(Pos2::ZERO, size);
    let form = Form::of(size);
    let mut found = Vec::new();
    for note in [NoteRow::Absent, NoteRow::Shown] {
        let rects = layout::shell(window, note);
        if rects.form != form {
            found.push(format!(
                "{note:?}: the shell is laid out as {:?}",
                rects.form
            ));
        }
        let asks = AskView::ALL.into_iter().flat_map(|view| {
            AskView::SIDE
                .into_iter()
                .map(move |side| Some((view, side)))
        });
        for ask in std::iter::once(None).chain(asks) {
            let parts = drawn(&rects, ask);
            found.extend(
                problems_of(window, form, ask, &parts)
                    .into_iter()
                    .map(|problem| format!("{note:?}: {problem}")),
            );
        }
    }
    found
}

#[test]
fn every_drawn_part_of_the_shell_fits_the_window_alone() {
    for [width, height] in NAMED_WINDOWS {
        let found = problems(vec2(width, height));
        assert!(found.is_empty(), "{width} x {height}: {found:#?}");
    }

    let any_window = (360.0f32..=2560.0, 400.0f32..=1600.0).prop_map(|(w, h)| vec2(w, h));
    let mut runner = TestRunner::new(Config {
        cases: 256,
        failure_persistence: None,
        ..Config::default()
    });
    runner
        .run(&any_window, |size| {
            let found = problems(size);
            prop_assert!(found.is_empty(), "{found:#?}");
            Ok(())
        })
        .unwrap();
}
