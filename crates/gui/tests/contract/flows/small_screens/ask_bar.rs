//! The Ask bar on a phone and on a tablet fits its window, and an ask sends the filters that were
//! chosen on the bar and in the sheet that holds the rest.

use contract::{AskView, Intent};
use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use gui::app::layout::{PHONE_WINDOW, TABLET_WINDOW};
use gui::testkit;
use gui::theme::{size, space};

use crate::flows::fit;
use crate::flows::recording;
use crate::flows::{
    Paper, Window, click, click_in, drawn, filters_of_the_last_ask, node, node_in, press,
    question_box, retype, shared,
};

const FILTER_BOXES: [&str; 5] = ["Media", "Document filter", "Authors", "Tags", "Category"];
/// The room between the last row of a phone's bar and the Answer under it: the margin at the foot
/// of the bar and the gap between the two.
const BAR_MARGIN_AND_GAP: f32 = 2.0 * space::SM;

#[test]
fn the_ask_bar_fits_a_phone_and_a_tablet_and_asks_with_the_filters_of_its_sheet() {
    let windows = [
        ("phone", PHONE_WINDOW, "Filters"),
        ("tablet", TABLET_WINDOW, "More filters"),
    ];
    for (name, window, more) in windows {
        let (mut harness, seen) = recording::open("black-scholes", window);
        testkit::settle(&mut harness);
        let bar = drawn(&harness).ask_bar;
        let library = &shared(&harness).library;
        let paper = Paper::of(library.catalogue.ready().expect("the library loaded"));

        let controls = assert_the_bar_fits(&harness, window);
        let names: Vec<&str> = controls.iter().map(|(name, _)| name.as_str()).collect();
        let mut on_the_bar = vec!["Question", "Mode", "Depth", "Ask", more];
        if window == TABLET_WINDOW {
            on_the_bar.extend(["Media", "Document filter"]);
        }
        for control in on_the_bar.iter().chain(&FILTER_BOXES) {
            let is_there = names.contains(control);
            let is_wanted = on_the_bar.contains(control);
            assert_eq!(is_there, is_wanted, "{name}: `{control}` on the bar");
        }
        if window == PHONE_WINDOW {
            let last_row_bottom = controls
                .iter()
                .map(|(_, rect)| rect.bottom())
                .fold(0.0, f32::max);
            assert!(
                drawn(&harness).answer.top() - last_row_bottom <= BAR_MARGIN_AND_GAP + fit::SLACK,
                "with no note the Answer starts right under the last row of the bar"
            );
        }
        testkit::save_png(&mut harness, &format!("{name}-ask-bar"));
        emptying_the_box_moves_nothing(&mut harness, name, window);

        // A filter on the bar is chosen where it stands. The rest are chosen in the sheet.
        let mut in_the_sheet = Vec::new();
        for filter in ["Media", "Authors", "Tags", "Category"] {
            if node_in(&harness, Role::ComboBox, filter, bar).is_some() {
                choose(&mut harness, filter, bar, &paper);
            } else {
                in_the_sheet.push(filter);
            }
        }
        click_in(&mut harness, Role::Button, more, bar);
        let heading = node(&harness, Role::Label, more).rect();
        let sheet = heading.union(node(&harness, Role::Button, "Done").rect());
        for filter in FILTER_BOXES {
            let places = harness
                .query_all_by_role_and_label(Role::ComboBox, filter)
                .filter(|found| {
                    [bar, sheet]
                        .iter()
                        .any(|area| area.contains(found.rect().center()))
                })
                .count();
            assert_eq!(
                places, 1,
                "{name}: `{filter}` stands once, on the bar or in the sheet"
            );
        }
        testkit::save_png(&mut harness, &format!("{name}-filters-sheet"));
        for filter in &in_the_sheet {
            choose(&mut harness, filter, sheet, &paper);
        }
        let count = node_in(&harness, Role::Button, more, bar).and_then(|button| button.value());
        let set_in_the_sheet = in_the_sheet.len().to_string();
        assert_eq!(
            count.as_deref(),
            Some(set_in_the_sheet.as_str()),
            "{name}: filters set in the sheet"
        );
        click(&mut harness, Role::Button, "Done");
        let modal = harness.ctx.memory(|memory| memory.top_modal_layer());
        assert!(modal.is_none(), "Done closes the sheet");

        assert_the_bar_fits(&harness, window);
        click_in(&mut harness, Role::Button, "Ask", bar);
        assert_eq!(
            filters_of_the_last_ask(&seen),
            Some(paper.filters()),
            "{name}: what the ask sends"
        );
    }
    a_library_note_stays_in_the_bar_of_a_phone();
}

/// The question box is emptied and typed again. Only the bar of a tablet says that the box is
/// empty, and no part of the window moves.
fn emptying_the_box_moves_nothing(harness: &mut Window, name: &str, window: [f32; 2]) {
    let question = shared(harness).ask.question.clone();
    let before = drawn(harness);
    click(harness, Role::TextInput, "Question");
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.step();
    harness.key_press(egui::Key::Backspace);
    testkit::settle(harness);
    assert_eq!(question_box(harness).as_deref(), Some(""));
    let controls = assert_the_bar_fits(harness, window);
    let shows_the_blank_note = controls
        .iter()
        .any(|(name, _)| name == "Type a question to ask.");
    // A phone's bar shows no blank note, so that it does not move while a person types.
    assert_eq!(
        shows_the_blank_note,
        window == TABLET_WINDOW,
        "{name}: the note for an empty box"
    );
    assert_eq!(
        drawn(harness),
        before,
        "{name}: the parts of the window do not move when the box is emptied"
    );
    retype(harness, "Question", &question);
}

/// The bar of a phone grows by a row only for a note about the library, and the note lies in it.
fn a_library_note_stays_in_the_bar_of_a_phone() {
    let mut harness = testkit::app("empty-library", PHONE_WINDOW);
    testkit::settle(&mut harness);
    let rects = drawn(&harness);
    let screen = fit::window_rect(PHONE_WINDOW);
    let note = node(
        &harness,
        Role::Label,
        "Your library is empty. Add a PDF on the Ingest tab.",
    )
    .rect();
    assert!(
        rects.ask_bar.contains_rect(note) && screen.contains_rect(note),
        "the note {note:?} lies in the bar {:?} and the window",
        rects.ask_bar
    );
    assert!(
        rects.answer.top() >= rects.ask_bar.bottom(),
        "the Answer {:?} starts under the bar {:?}",
        rects.answer,
        rects.ask_bar
    );

    // A library that did not load has a button in its note, and a finger presses it.
    let mut harness = testkit::app("stores-down", PHONE_WINDOW);
    harness
        .state_mut()
        .push(Intent::ShowAskView(AskView::Answer));
    testkit::settle(&mut harness);
    let rects = drawn(&harness);
    let retry = node_in(&harness, Role::Button, "Try again", rects.ask_bar)
        .expect("the note has a button that reads the library again")
        .rect();
    assert!(
        rects.ask_bar.contains_rect(retry) && screen.contains_rect(retry),
        "Try again {retry:?} lies in the bar {:?} and the window",
        rects.ask_bar
    );
    fit::assert_finger_sized("Try again", retry, size::CONTROL_MD);
    assert!(rects.answer.top() >= rects.ask_bar.bottom());
}

/// Every control of the bar lies in the bar and in the window, and none covers another. On a
/// phone each one is tall enough for a finger. Returns their names with their rectangles.
fn assert_the_bar_fits(harness: &Window, window: [f32; 2]) -> Vec<(String, egui::Rect)> {
    let bar = drawn(harness).ask_bar;
    let touched = [Role::TextInput, Role::ComboBox, Role::Button];
    let roles = touched.into_iter().chain([Role::CheckBox, Role::Label]);
    let controls: Vec<(String, Role, egui::Rect)> = roles
        .flat_map(|role| {
            harness
                .query_all_by_role(role)
                .map(move |node| (role, node))
        })
        .filter(|(_, control)| bar.expand(fit::SLACK).contains(control.rect().center()))
        .map(|(role, control)| {
            // A label keeps its words as its value.
            let name = control.accesskit_node().label().or_else(|| control.value());
            let name = name.unwrap_or_default();
            (name, role, control.rect())
        })
        .collect();
    let parts: Vec<(String, egui::Rect)> = controls
        .iter()
        .map(|(name, _, rect)| (name.clone(), *rect))
        .collect();
    fit::assert_inside_and_apart(&parts, bar, fit::window_rect(window));
    if window == PHONE_WINDOW {
        for (name, role, rect) in &controls {
            if touched.contains(role) {
                fit::assert_finger_sized(name, *rect, size::CONTROL_MD);
            }
        }
    }
    parts
}

/// Opens the list `filter` in `area` and picks what `paper` stands for.
fn choose(harness: &mut Window, filter: &str, area: egui::Rect, paper: &Paper) {
    click_in(harness, Role::ComboBox, filter, area);
    match filter {
        "Media" => click(harness, Role::Button, &paper.title),
        "Authors" => click(harness, Role::Button, &paper.author),
        "Tags" => {
            click(harness, Role::CheckBox, &paper.tag);
            press(harness, egui::Modifiers::NONE, egui::Key::Escape);
        }
        "Category" => click(harness, Role::Button, "Paper"),
        other => panic!("`{other}` is not a filter to choose"),
    }
}
