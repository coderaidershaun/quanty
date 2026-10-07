//! No key and no control on the Ask screen leads to a part of the app that is not built: not a
//! page, not the help sheet, not a command whose backend only says it is not built.

use std::collections::{HashMap, HashSet};

use eframe::egui;
use eframe::egui::accesskit::{Action, Role};
use egui_kittest::kittest::{By, NodeT as _, Queryable as _};
use gui::app::layout::DEFAULT_WINDOW;
use gui::backend::fake::{self, Fake};
use gui::contract::{Chord, Command, Failure, Intent, KeyName, SHORTCUTS, Tab};
use gui::state::Quit;
use gui::testkit;

use super::recording::{self, Seen};
use super::{Window, failures, press, shared};

/// Names that no node of the Ask screen may have, because each is a part that is not built.
const ABSENT_NAMES: [&str; 7] = [
    "Health",
    "Notices",
    "Help",
    "Check again",
    "Ingest a chapter",
    "Go to Ingest",
    "Open Ingest",
];

/// More controls than any scene has. A screen that makes new names for ever stops here.
const MOST_CONTROLS: usize = 600;

fn key_of(key: KeyName) -> egui::Key {
    match key {
        KeyName::Num1 => egui::Key::Num1,
        KeyName::Num2 => egui::Key::Num2,
        KeyName::Num3 => egui::Key::Num3,
        KeyName::J => egui::Key::J,
        KeyName::K => egui::Key::K,
        KeyName::R => egui::Key::R,
        KeyName::S => egui::Key::S,
        KeyName::Slash => egui::Key::Slash,
        KeyName::Period => egui::Key::Period,
        KeyName::Escape => egui::Key::Escape,
        KeyName::OpenBracket => egui::Key::OpenBracket,
        KeyName::CloseBracket => egui::Key::CloseBracket,
    }
}

fn modifiers_of(chord: Chord) -> egui::Modifiers {
    let mut modifiers = egui::Modifiers::NONE;
    if chord.command {
        modifiers |= egui::Modifiers::COMMAND;
    }
    if chord.shift {
        modifiers |= egui::Modifiers::SHIFT;
    }
    modifiers
}

/// The commands the app must send as it opens: one load of the catalogue, and what the scene's
/// own opening intents ask for.
fn assert_start_up_sent_only_its_own_commands(scene: &str, seen: &Seen) {
    let opening = Fake::scene(scene, &testkit::samples_folder())
        .unwrap_or_else(|error| panic!("scene `{scene}`: {error}"))
        .opening();
    let asks = opening
        .iter()
        .filter(|intent| matches!(intent, Intent::Ask(_)))
        .count();
    let pages = opening
        .iter()
        .filter(|intent| matches!(intent, Intent::OpenSource { .. }))
        .count();
    let counted = |is: fn(&Command) -> bool| seen.count(is);
    assert_eq!(
        counted(|c| matches!(c, Command::LoadCatalogue { .. })),
        1,
        "`{scene}`"
    );
    assert_eq!(
        counted(|c| matches!(c, Command::Ask { .. })),
        asks,
        "`{scene}`"
    );
    assert_eq!(
        counted(|c| matches!(c, Command::LoadPage { .. })),
        pages,
        "`{scene}`"
    );
    assert_eq!(
        seen.all().len(),
        1 + asks + pages,
        "`{scene}` sent a command that its opening does not ask for: {:?}",
        seen.all()
    );
}

/// The role and name of every enabled node that a click can press, with how many nodes before it
/// have the same role and name.
fn pressable(harness: &Window) -> Vec<(Role, String, usize)> {
    let can_be_pressed = |node: &egui_kittest::kittest::AccessKitNode<'_>| {
        node.data().supports_action(Action::Click)
            && !node.is_disabled()
            && !node.is_hidden()
            && node.label().is_some()
            && node.bounding_box().is_some()
    };
    let mut before: HashMap<(Role, String), usize> = HashMap::new();
    let mut found = Vec::new();
    for node in harness.query_all(By::new().predicate(can_be_pressed)) {
        let (role, name) = (
            node.accesskit_node().role(),
            node.accesskit_node().label().unwrap_or_default(),
        );
        let count = before.entry((role, name.clone())).or_default();
        found.push((role, name, *count));
        *count += 1;
    }
    found
}

/// Clicks the node again, when it is still there and still enabled.
fn click_if_still_there(harness: &mut Window, role: Role, name: &str, place: usize) {
    let Some(node) = harness.query_all_by_role_and_label(role, name).nth(place) else {
        return;
    };
    if node.accesskit_node().is_disabled() || node.accesskit_node().bounding_box().is_none() {
        return;
    }
    node.click();
    testkit::settle(harness);
}

/// Presses every control once, and every control that a press brings on screen. A tab is pressed
/// only when nothing else on screen is left, so what a tab shows is pressed before the tab is
/// left. The lists are left to `press_every_row_of_every_list`.
fn press_every_control(harness: &mut Window, scene: &str) {
    let mut pressed: HashSet<(Role, String, usize)> = HashSet::new();
    loop {
        let next = pressable(harness)
            .into_iter()
            .filter(|control| control.0 != Role::ComboBox && !pressed.contains(control))
            .min_by_key(|(role, ..)| *role == Role::Tab);
        let Some((role, name, place)) = next else {
            return;
        };
        click_if_still_there(harness, role, &name, place);
        pressed.insert((role, name, place));
        assert!(
            pressed.len() <= MOST_CONTROLS,
            "`{scene}` keeps bringing new controls on screen: {pressed:?}"
        );
    }
}

/// The rows of an open list: the controls that are there now and were not there before.
fn rows_of_open_list(harness: &Window, closed: &HashSet<(Role, String)>) -> Vec<(Role, String)> {
    pressable(harness)
        .into_iter()
        .map(|(role, name, _)| (role, name))
        .filter(|control| !closed.contains(control))
        .collect()
}

/// Opens each list, and presses each of its rows: a row is a control that only an open list has.
fn press_every_row_of_every_list(harness: &mut Window) {
    // A list that the last click left open is shut first, so that its rows are not taken for
    // controls that are always there.
    harness.key_press(egui::Key::Escape);
    harness.step();
    testkit::settle(harness);
    let closed: HashSet<(Role, String)> = pressable(harness)
        .into_iter()
        .map(|(role, name, _)| (role, name))
        .collect();
    let lists = pressable(harness)
        .into_iter()
        .filter(|(role, ..)| *role == Role::ComboBox);
    for (role, name, place) in lists.collect::<Vec<_>>() {
        // A list that an earlier click left open is shut by this click, so a second click opens it.
        click_if_still_there(harness, role, &name, place);
        let mut rows = rows_of_open_list(harness, &closed);
        if rows.is_empty() {
            click_if_still_there(harness, role, &name, place);
            rows = rows_of_open_list(harness, &closed);
        }
        for (row_role, row) in rows {
            // Choosing a row closes its list, so the list is opened again for the next one.
            if harness
                .query_all_by_role_and_label(row_role, &row)
                .next()
                .is_none()
            {
                click_if_still_there(harness, role, &name, place);
            }
            click_if_still_there(harness, row_role, &row, 0);
        }
    }
}

/// Every `CheckHealth` that the app sent after the start-up was the reducer's own recheck after
/// a search that worked, so an ask came before it, and after the one before it.
fn assert_each_health_check_follows_an_ask(scene: &str, seen: &Seen) {
    let mut asked = false;
    for command in seen.all() {
        match command {
            Command::Ask { .. } => asked = true,
            Command::CheckHealth { .. } => {
                assert!(asked, "`{scene}` checked health with no ask before it");
                asked = false;
            }
            _ => {}
        }
    }
}

fn assert_nothing_unbuilt_was_reached(harness: &Window, scene: &str, seen: &Seen) {
    for command in seen.all() {
        assert!(
            !matches!(
                command,
                Command::SetLabels { .. }
                    | Command::DeleteDocument { .. }
                    | Command::Preflight { .. }
                    | Command::Ingest { .. }
            ),
            "`{scene}` sent {command:?}, a command of a part that is not built"
        );
    }
    let shared = shared(harness);
    assert_eq!(shared.tab, Tab::Ask, "`{scene}` left the Ask screen");
    assert!(!shared.help_open, "`{scene}` opened the help sheet");
    assert_eq!(shared.quit, Quit::No);
    let not_built = Failure::not_built("anything").hint;
    let health_failures =
        gui::contract::Service::ALL
            .into_iter()
            .filter_map(|service| match shared.health.of(service) {
                gui::contract::ServiceState::Down(failure) => Some(failure),
                _ => None,
            });
    for failure in failures(shared).into_iter().chain(health_failures) {
        assert_ne!(
            failure.hint, not_built,
            "`{scene}` shows a part that is not built"
        );
    }
    assert!(
        harness
            .query_all_by_label_contains("not built")
            .next()
            .is_none(),
        "`{scene}` says that something is not built"
    );
    for name in ABSENT_NAMES {
        assert!(
            harness.query_all_by_label(name).next().is_none(),
            "`{scene}` has a node named `{name}`"
        );
    }
    for name in ["Library", "Ingest"] {
        assert!(
            harness
                .query_all_by_role_and_label(Role::Tab, name)
                .next()
                .is_none(),
            "`{scene}` has a tab named `{name}`"
        );
    }
}

fn every_key_and_control_of(scene: &str) {
    let (mut harness, seen) = recording::open(scene, DEFAULT_WINDOW);
    testkit::settle(&mut harness);
    assert_start_up_sent_only_its_own_commands(scene, &seen);

    for row in &SHORTCUTS {
        // A key that the box takes as text does not reach the shortcut, so the box is left
        // first, as a person leaves it.
        if harness
            .query_all_by_role_and_label(Role::TextInput, "Question")
            .any(|node| node.accesskit_node().is_focused())
        {
            press(&mut harness, egui::Modifiers::NONE, egui::Key::Escape);
        }
        press(&mut harness, modifiers_of(row.chord), key_of(row.chord.key));
    }
    assert_nothing_unbuilt_was_reached(&harness, scene, &seen);
    assert!(
        seen.all()
            .iter()
            .all(|command| !matches!(command, Command::CheckHealth { .. })),
        "`{scene}`: a key sent a health check"
    );

    press_every_control(&mut harness, scene);
    press_every_row_of_every_list(&mut harness);
    assert_nothing_unbuilt_was_reached(&harness, scene, &seen);
    assert_each_health_check_follows_an_ask(scene, &seen);
}

#[test]
fn nothing_on_the_ask_screen_leads_to_a_part_that_is_not_built() {
    for scene in fake::scenes().iter().filter(|scene| scene.rests) {
        every_key_and_control_of(scene.name);
    }
}
