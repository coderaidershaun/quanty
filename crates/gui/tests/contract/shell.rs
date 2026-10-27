//! Checks the window around the panels: what Back does in a wide window and with a sheet open, how
//! the top bar fits every form, and the switch between the dark and the light palette.

use contract::{AskView, Intent, Tab};
use eframe::egui;
use eframe::egui::Rect;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use gui::app::layout::{
    DEFAULT_WINDOW, Form, MIN_WINDOW, PHONE_LANDSCAPE_WINDOW, PHONE_WINDOW, TABLET_WINDOW,
};
use gui::testkit;
use gui::theme::{self, Palette, Scheme, size};

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

/// The name of the switch while the window is dark, and while it is light.
const SWITCH_TO_LIGHT: &str = "Switch to light mode";
const SWITCH_TO_DARK: &str = "Switch to dark mode";

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
/// overlap. The model control is one button or the two dropdowns, and the Library has none. The
/// switch between the palettes stands in the bar of a wide or a medium window, and a medium one
/// is finger-sized. A compact window has it in the footer's popup, not in the bar.
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
    let switch = find(Role::Button, SWITCH_TO_LIGHT);
    match Form::of(egui::Vec2::from(window)) {
        Form::Wide => {
            assert!(
                switch.is_some(),
                "{window:?}: no switch is in the bar {bar:?}"
            );
        }
        Form::Medium => {
            let (_, rect) = switch
                .as_ref()
                .unwrap_or_else(|| panic!("{window:?}: no switch is in the bar {bar:?}"));
            assert!(
                rect.height() >= size::CONTROL_MD,
                "{window:?}: the switch is {} tall, and a finger needs {}",
                rect.height(),
                size::CONTROL_MD
            );
        }
        Form::Compact => {
            assert!(switch.is_none(), "{window:?}: a compact bar has the switch");
        }
    }
    parts.extend(switch);
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
        DEFAULT_WINDOW,
        MIN_WINDOW,
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

/// How far the pixel of the renderer may be from the colour of the palette: the conversion from
/// the colour space of the screen can round by one.
const COLOUR_ROUNDING: i16 = 1;

#[test]
fn the_app_opens_dark_when_nothing_was_chosen() {
    let mut harness = testkit::app("idle", DEFAULT_WINDOW);
    testkit::settle(&mut harness);
    assert_eq!(shared(&harness).scheme, Scheme::Dark);
    assert_eq!(theme::scheme(&harness.ctx), Scheme::Dark);

    // The gutter at the corner of the window holds nothing but the desk.
    let scale = harness.ctx.pixels_per_point();
    let frame = harness.render().expect("the test renderer draws the frame");
    let corner = (4.0 * scale) as u32;
    let drawn = frame.get_pixel(corner, corner).0;
    let desk = Palette::DARK.desk.to_array();
    for channel in 0..3 {
        assert!(
            (i16::from(drawn[channel]) - i16::from(desk[channel])).abs() <= COLOUR_ROUNDING,
            "the corner of the window is {drawn:?}, and the dark desk is {desk:?}"
        );
    }
}

/// Opens the footer's popup with a tap on the slim row, as a person does on a phone.
fn open_the_footer_popup(harness: &mut Window) {
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
}

/// The buttons of the footer's popup besides the switch: the two accounts and their links, which
/// stand above the slim row.
fn account_buttons_in_the_popup(harness: &Window) -> Vec<(String, Rect)> {
    let footer = panels(PHONE_WINDOW).footer;
    harness
        .query_all_by_role(Role::Button)
        .filter_map(|button| {
            let name = button.accesskit_node().label()?;
            let is_account = name.starts_with("Google Gemini:")
                || name.starts_with("Jev:")
                || name.starts_with("Open the ");
            (is_account && !footer.contains(button.rect().center()))
                .then(|| (name.to_string(), button.rect()))
        })
        .collect()
}

fn assert_scheme(harness: &Window, scheme: Scheme, when: &str) {
    assert_eq!(shared(harness).scheme, scheme, "{when}: the choice");
    assert_eq!(theme::scheme(&harness.ctx), scheme, "{when}: what is drawn");
}

/// The switch, wherever the window keeps it, found by the name it has for the scheme drawn.
fn press_the_switch(harness: &mut Window, window: [f32; 2], name: &str) {
    click_in(harness, Role::Button, name, fit::window_rect(window));
}

fn press_escape(harness: &mut Window) {
    harness.key_press(egui::Key::Escape);
    harness.step();
    testkit::settle(harness);
}

#[test]
fn the_switch_stands_in_the_chrome_on_every_tab_and_flips_the_palette() {
    // A phone keeps the switch in the footer's popup, and every tab can open that popup.
    let mut harness = testkit::app("black-scholes", PHONE_WINDOW);
    testkit::settle(&mut harness);
    let screen = fit::window_rect(PHONE_WINDOW);
    for (_, tab) in TABS {
        click_in(&mut harness, Role::Tab, tab, panels(PHONE_WINDOW).top_bar);
        open_the_footer_popup(&mut harness);
        let switch = node_in(&harness, Role::Button, SWITCH_TO_LIGHT, screen)
            .unwrap_or_else(|| panic!("{tab}: the footer popup has no switch"))
            .rect();
        assert!(
            screen.contains_rect(switch),
            "{tab}: the switch {switch:?} is outside the window"
        );
        assert!(
            switch.height() >= size::CONTROL_MD,
            "{tab}: the switch is {} tall, and a finger needs {}",
            switch.height(),
            size::CONTROL_MD
        );
        for (name, other) in account_buttons_in_the_popup(&harness) {
            let both = switch.intersect(other);
            assert!(
                both.width() <= 0.5 || both.height() <= 0.5,
                "{tab}: the switch {switch:?} covers `{name}` {other:?}"
            );
        }
        go_back(&mut harness);
        assert!(!is_a_popup_open(&harness), "{tab}: Back closes the popup");
    }

    // A click turns the palette over, a second click turns it back, and neither Back nor Escape
    // turns it, in a wide window, a medium one and a phone.
    for window in [DEFAULT_WINDOW, TABLET_WINDOW, PHONE_WINDOW] {
        let mut harness = testkit::app("black-scholes", window);
        testkit::settle(&mut harness);
        let is_phone = window == PHONE_WINDOW;
        if is_phone {
            open_the_footer_popup(&mut harness);
        }
        assert_scheme(&harness, Scheme::Dark, &format!("{window:?} at the start"));

        press_the_switch(&mut harness, window, SWITCH_TO_LIGHT);
        assert_scheme(
            &harness,
            Scheme::Light,
            &format!("{window:?} after a click"),
        );
        assert!(
            has(&harness, Role::Button, SWITCH_TO_DARK),
            "{window:?}: the switch is not named for the way back"
        );

        press_the_switch(&mut harness, window, SWITCH_TO_DARK);
        assert_scheme(
            &harness,
            Scheme::Dark,
            &format!("{window:?} after two clicks"),
        );
        assert!(
            has(&harness, Role::Button, SWITCH_TO_LIGHT),
            "{window:?}: the switch is not named for the way forward"
        );

        go_back(&mut harness);
        assert_scheme(&harness, Scheme::Dark, &format!("{window:?} after Back"));
        if is_phone {
            assert!(!is_a_popup_open(&harness), "Back closes the popup");
            open_the_footer_popup(&mut harness);
        }
        press_escape(&mut harness);
        assert_scheme(&harness, Scheme::Dark, &format!("{window:?} after Escape"));
    }
}
