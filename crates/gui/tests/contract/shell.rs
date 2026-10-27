//! Checks the window around the panels: what Back does in a wide window and with a sheet open, and
//! how the top bar fits every form.

use contract::{AskView, Intent, Tab};
use eframe::egui;
use eframe::egui::Rect;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use gui::app::layout::{DEFAULT_WINDOW, PHONE_LANDSCAPE_WINDOW, PHONE_WINDOW, TABLET_WINDOW};
use gui::testkit;

use crate::flows::fit;
use crate::flows::{Wheel, Window, click_in, has, node_in, panels, scroll_to, shared};

/// The top-bar tabs, in the order they stand in the bar.
const TABS: [(Tab, &str); 4] = [
    (Tab::Ask, "Ask"),
    (Tab::Library, "Library"),
    (Tab::Ingest, "Ingest"),
    (Tab::Datasets, "Datasets"),
];

/// A medium window wide enough to keep both dropdowns in the bar.
const MEDIUM_WITH_ROOM: [f32; 2] = [1000.0, 700.0];

const NOTES: &str = "Quanty Sample Notes";

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
    let find = |role, name: &'static str| {
        node_in(harness, role, name, bar).map(|node| (name.to_owned(), node.rect()))
    };
    let mut parts: Vec<(String, Rect)> = Vec::new();
    for (_, name) in TABS {
        let found = find(Role::Tab, name)
            .unwrap_or_else(|| panic!("{window:?}: no tab `{name}` is in the bar {bar:?}"));
        parts.push(found);
    }
    parts.extend(find(Role::Label, "Quanty"));
    let model_control: Vec<(String, Rect)> = [
        (Role::Button, "Model choices"),
        (Role::ComboBox, "Model"),
        (Role::ComboBox, "Thinking level"),
    ]
    .into_iter()
    .filter_map(|(role, name)| find(role, name))
    .collect();
    let names: Vec<&str> = model_control
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    if tab == Tab::Library {
        assert!(names.is_empty(), "{window:?}: {names:?} on the Library");
    } else {
        assert!(
            names == ["Model choices"] || names == ["Model", "Thinking level"],
            "{window:?}: the model control is {names:?} on {tab:?}"
        );
    }
    parts.extend(model_control);
    fit::assert_inside_and_apart(&parts, bar, fit::window_rect(window));
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

fn tap(harness: &mut Window, role: Role, name: &str) {
    click_in(harness, role, name, fit::window_rect(PHONE_WINDOW));
}

fn is_a_popup_open(harness: &Window) -> bool {
    egui::Popup::is_any_open(&harness.ctx)
}

fn is_a_sheet_open(harness: &Window) -> bool {
    harness
        .ctx
        .memory(|memory| memory.top_modal_layer())
        .is_some()
}

/// A sheet or popup of a phone, how a person opens it, and how a test sees that it is open.
struct Overlay {
    name: &'static str,
    open: fn(&mut Window),
    is_shown: fn(&Window) -> bool,
}

const OVERLAYS: [Overlay; 5] = [
    Overlay {
        name: "the model sheet",
        open: |harness| tap(harness, Role::Button, "Model choices"),
        is_shown: |harness| has(harness, Role::ComboBox, "Model"),
    },
    Overlay {
        name: "the footer popup",
        open: |harness| {
            let footer = panels(PHONE_WINDOW).footer;
            harness
                .query_all_by_role(Role::Button)
                .find(|button| {
                    let name = button.accesskit_node().label().unwrap_or_default();
                    name.starts_with("Google Gemini:") && footer.contains(button.rect().center())
                })
                .expect("the slim footer has a row for each account")
                .click();
            testkit::settle(harness);
        },
        is_shown: is_a_popup_open,
    },
    Overlay {
        name: "the Filters sheet",
        open: |harness| tap(harness, Role::Button, "Filters"),
        is_shown: |harness| has(harness, Role::Button, "Done"),
    },
    Overlay {
        name: "the delete sheet",
        open: |harness| {
            tap(harness, Role::Tab, "Library");
            tap(harness, Role::Button, NOTES);
            tap(harness, Role::Button, "About this media");
            tap(harness, Role::Button, &format!("Delete media {NOTES}"));
        },
        is_shown: |harness| has(harness, Role::Label, &format!("Delete the media {NOTES}?")),
    },
    Overlay {
        name: "the Share menu",
        open: |harness| tap(harness, Role::Button, "Share"),
        is_shown: |harness| has(harness, Role::Button, "Copy as Markdown"),
    },
];

fn go_back(harness: &mut Window) {
    harness.state_mut().push(Intent::Back);
    testkit::settle(harness);
}

#[test]
fn back_closes_the_sheet_on_top_and_does_nothing_else() {
    for overlay in OVERLAYS {
        let mut harness = testkit::app("black-scholes", PHONE_WINDOW);
        testkit::settle(&mut harness);
        (overlay.open)(&mut harness);
        assert!(
            (overlay.is_shown)(&harness),
            "{} did not open",
            overlay.name
        );
        let before = shared(&harness).clone();
        assert!(
            harness.state().can_go_back(),
            "{}: Back has a sheet to close",
            overlay.name
        );

        go_back(&mut harness);
        assert!(
            !(overlay.is_shown)(&harness),
            "{} is still open after Back",
            overlay.name
        );
        assert!(
            !is_a_popup_open(&harness) && !is_a_sheet_open(&harness),
            "{}: something is still open",
            overlay.name
        );
        assert_eq!(
            *shared(&harness),
            before,
            "{}: Back changed more than the sheet",
            overlay.name
        );
    }

    // A window that shows every view at once has a sheet to close too.
    let mut harness = testkit::app("black-scholes", DEFAULT_WINDOW);
    testkit::settle(&mut harness);
    let screen = fit::window_rect(DEFAULT_WINDOW);
    click_in(&mut harness, Role::Button, "Share", screen);
    assert!(harness.state().can_go_back(), "the menu is open");
    go_back(&mut harness);
    assert!(!has(&harness, Role::Button, "Copy as Markdown"));
    assert!(!harness.state().can_go_back(), "nothing else is left");

    // A list that is open inside the model sheet closes first, and the sheet stays.
    let mut harness = testkit::app("black-scholes", PHONE_WINDOW);
    testkit::settle(&mut harness);
    tap(&mut harness, Role::Button, "Model choices");
    tap(&mut harness, Role::ComboBox, "Model");
    assert!(is_a_popup_open(&harness), "the list of models is open");
    go_back(&mut harness);
    assert!(!is_a_popup_open(&harness), "Back closed the list");
    assert!(has(&harness, Role::ComboBox, "Model"), "the sheet stays");
    go_back(&mut harness);
    assert!(!has(&harness, Role::ComboBox, "Model"), "then the sheet");
}
