//! Checks the window around the panels: what Back does in a wide window, and how the top bar
//! fits every form.

use contract::{AskView, Intent, Tab};
use eframe::egui::accesskit::Role;
use eframe::egui::{Pos2, Rect, Vec2};
use gui::app::layout::{DEFAULT_WINDOW, PHONE_LANDSCAPE_WINDOW, PHONE_WINDOW, TABLET_WINDOW};
use gui::testkit;

use crate::flows::{Wheel, Window, click_in, node_in, panels, scroll_to, shared};

/// The top-bar tabs, in the order they stand in the bar.
const TABS: [(Tab, &str); 4] = [
    (Tab::Ask, "Ask"),
    (Tab::Library, "Library"),
    (Tab::Ingest, "Ingest"),
    (Tab::Datasets, "Datasets"),
];

/// A medium window wide enough to keep both dropdowns in the bar.
const MEDIUM_WITH_ROOM: [f32; 2] = [1000.0, 700.0];

#[test]
fn a_wide_window_has_nothing_to_go_back_to_after_a_citation() {
    let mut harness = testkit::app("black-scholes", DEFAULT_WINDOW);
    testkit::settle(&mut harness);
    let answer = panels(DEFAULT_WINDOW).answer;
    scroll_to(
        &mut harness,
        answer,
        Wheel::Down,
        Role::Button,
        "Citation 1",
    );
    node_in(&harness, Role::Button, "Citation 1", answer)
        .expect("a citation chip is in the Answer")
        .click();
    testkit::settle(&mut harness);
    assert_eq!(shared(&harness).ask_view, AskView::Source);
    assert!(
        !harness.state().can_go_back(),
        "every view is on screen already, so Back has nothing to change"
    );

    harness.state_mut().push(Intent::Back);
    testkit::settle(&mut harness);
    assert_eq!(shared(&harness).ask_view, AskView::Source);
    assert_eq!(shared(&harness).tab, Tab::Ask);
}

/// Every part of the top bar that is on screen lies inside the bar and the window, and no two
/// overlap. The model control is one button or the two dropdowns, and the Library has none.
fn assert_top_bar_fits(harness: &Window, window: [f32; 2], tab: Tab) {
    let bar = panels(window).top_bar;
    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::from(window));
    let find = |role, name: &'static str| {
        node_in(harness, role, name, bar).map(|node| (name, node.rect()))
    };
    let mut parts: Vec<(&str, Rect)> = Vec::new();
    for (_, name) in TABS {
        let found = find(Role::Tab, name)
            .unwrap_or_else(|| panic!("{window:?}: no tab `{name}` is in the bar {bar:?}"));
        parts.push(found);
    }
    parts.extend(find(Role::Label, "Quanty"));
    let model_control: Vec<(&str, Rect)> = [
        (Role::Button, "Model choices"),
        (Role::ComboBox, "Model"),
        (Role::ComboBox, "Thinking level"),
    ]
    .into_iter()
    .filter_map(|(role, name)| find(role, name))
    .collect();
    let names: Vec<&str> = model_control.iter().map(|(name, _)| *name).collect();
    if tab == Tab::Library {
        assert!(names.is_empty(), "{window:?}: {names:?} on the Library");
    } else {
        assert!(
            names == ["Model choices"] || names == ["Model", "Thinking level"],
            "{window:?}: the model control is {names:?} on {tab:?}"
        );
    }
    parts.extend(model_control);

    // SMELL: the footer test of a phone and the bottom navigation test repeat this check that
    // parts lie inside their room and apart. One helper beside the other shared test helpers would
    // serve all three.
    for (name, rect) in &parts {
        assert!(
            bar.contains_rect(*rect) && screen.contains_rect(*rect),
            "{window:?}: `{name}` {rect:?} is outside the bar {bar:?}"
        );
    }
    for (index, (name, rect)) in parts.iter().enumerate() {
        for (other_name, other) in &parts[index + 1..] {
            let overlap = rect.intersect(*other);
            assert!(
                overlap.width() <= 0.0 || overlap.height() <= 0.0,
                "{window:?}: `{name}` {rect:?} overlaps `{other_name}` {other:?}"
            );
        }
    }
}

#[test]
fn the_top_bar_fits_every_form() {
    for window in [
        PHONE_WINDOW,
        PHONE_LANDSCAPE_WINDOW,
        TABLET_WINDOW,
        [360.0, 640.0],
        MEDIUM_WITH_ROOM,
    ] {
        let mut harness = testkit::app("idle", window);
        testkit::settle(&mut harness);
        assert_top_bar_fits(&harness, window, Tab::Ask);
        testkit::save_png(
            &mut harness,
            &format!("top-bar-{}x{}", window[0], window[1]),
        );

        let bar = panels(window).top_bar;
        for (tab, name) in [TABS[1], TABS[2], TABS[3], TABS[0]] {
            click_in(&mut harness, Role::Tab, name, bar);
            assert_eq!(shared(&harness).tab, tab, "{window:?}: a click on `{name}`");
            assert_top_bar_fits(&harness, window, tab);
        }
    }
}
