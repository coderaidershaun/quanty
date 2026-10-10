//! No key and no control of the app leads to a part of it that is not built: not a page, not the
//! help sheet, not a command whose backend only says it is not built. A Delete control and the two
//! delete commands are built, so they are pressed like any other control.

use std::collections::{HashMap, HashSet};

use eframe::egui;
use eframe::egui::accesskit::{Action, NodeId, Role};
use egui_kittest::kittest::{AccessKitNode, By, NodeT as _, Queryable as _};
use gui::app::layout::DEFAULT_WINDOW;
use gui::backend::fake::{self, Fake};
use gui::contract::{Chord, Command, Failure, Intent, KeyName, SHORTCUTS};
use gui::state::{MediaDelete, Quit};
use gui::testkit;

use super::recording::{self, Seen};
use super::{Window, failures, is_open_tab, node, press, shared};

/// Names that no node of the app may have, because each is a part that is not built.
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
    let checks = opening
        .iter()
        .filter(|intent| matches!(intent, Intent::CheckIngest(_)))
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
        counted(|c| matches!(c, Command::Preflight { .. })),
        checks,
        "`{scene}`"
    );
    assert_eq!(
        seen.all().len(),
        1 + asks + pages + checks,
        "`{scene}` sent a command that its opening does not ask for: {:?}",
        seen.all()
    );
}

/// A control that a click can press.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Control {
    role: Role,
    name: String,
    /// How many pressable controls of this role and name come before it, as the click finds it.
    place: usize,
    /// True for a control of the sheet that is open over the page. The `Cancel` of a sheet and
    /// the `Cancel` of a form are two controls, so each is pressed once. A row of an open list is
    /// drawn over the page as a sheet is, so it is true for such a row too.
    is_on_sheet: bool,
}

/// The child of the root that holds the node. A sheet is an area of its own under the root, so a
/// control of the sheet ends at another node than a control of the page does.
fn top_of(node: &AccessKitNode<'_>) -> NodeId {
    let mut top = *node;
    while let Some(parent) = top.parent() {
        if parent.is_root() {
            break;
        }
        top = parent;
    }
    top.locate().0
}

/// Every enabled node that a click can press.
fn pressable(harness: &Window) -> Vec<Control> {
    let can_be_pressed = |node: &AccessKitNode<'_>| {
        node.data().supports_action(Action::Click)
            && !node.is_disabled()
            && !node.is_hidden()
            && node.label().is_some()
            && node.bounding_box().is_some()
    };
    let page = top_of(&node(harness, Role::Tab, "Ask").accesskit_node());
    let mut before: HashMap<(Role, String), usize> = HashMap::new();
    let mut found = Vec::new();
    for node in harness.query_all(By::new().predicate(can_be_pressed)) {
        let (role, name) = (
            node.accesskit_node().role(),
            node.accesskit_node().label().unwrap_or_default(),
        );
        let count = before.entry((role, name.clone())).or_default();
        found.push(Control {
            role,
            name,
            place: *count,
            is_on_sheet: top_of(&node.accesskit_node()) != page,
        });
        *count += 1;
    }
    found
}

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

/// egui sets the layer of a sheet at the start of the next frame, so two frames run first.
fn is_a_sheet_open(harness: &mut Window) -> bool {
    harness.step();
    harness.step();
    harness
        .ctx
        .memory(|memory| memory.top_modal_layer().is_some())
}

/// A click on a control under a sheet lands on the sheet or on its backdrop, so while a sheet is
/// open only its own controls are pressed. Escape closes what is left of it.
fn close_the_sheet_with_escape(harness: &mut Window, scene: &str) {
    press(harness, egui::Modifiers::NONE, egui::Key::Escape);
    assert!(
        !is_a_sheet_open(harness),
        "`{scene}`: Escape did not close the sheet"
    );
}

/// A tab is pressed only when nothing else on screen is left, so what a tab shows is pressed
/// before the tab is left. The lists are left to `press_every_row_of_every_list`.
///
/// A button named `Read` is pressed after every other control that is not a tab. It opens the
/// Ask tab, and the Library tab is never pressed a second time, so the rest of the Library page
/// would never be pressed if `Read` came first. The rule goes by name: a button that is renamed
/// makes this test weaker and nothing fails.
fn press_every_control(harness: &mut Window, scene: &str) {
    let mut pressed: HashSet<Control> = HashSet::new();
    loop {
        let is_sheet_open = is_a_sheet_open(harness);
        let next = pressable(harness)
            .into_iter()
            .filter(|control| {
                control.role != Role::ComboBox
                    && !pressed.contains(control)
                    && (control.is_on_sheet || !is_sheet_open)
            })
            .min_by_key(|control| {
                (
                    control.role == Role::Tab,
                    control.role == Role::Button && control.name == "Read",
                )
            });
        let Some(control) = next else {
            if !is_sheet_open {
                return;
            }
            close_the_sheet_with_escape(harness, scene);
            continue;
        };
        click_if_still_there(harness, control.role, &control.name, control.place);
        pressed.insert(control);
        assert!(
            pressed.len() <= MOST_CONTROLS,
            "`{scene}` keeps bringing new controls on screen: {pressed:?}"
        );
    }
}

fn rows_of_open_list(harness: &Window, closed: &HashSet<(Role, String)>) -> Vec<(Role, String)> {
    pressable(harness)
        .into_iter()
        .map(|control| (control.role, control.name))
        .filter(|control| !closed.contains(control))
        .collect()
}

/// A row is a control that only an open list has.
fn press_every_row_of_every_list(harness: &mut Window) {
    // A list that the last click left open is shut first, so that its rows are not taken for
    // controls that are always there.
    harness.key_press(egui::Key::Escape);
    harness.step();
    testkit::settle(harness);
    let closed: HashSet<(Role, String)> = pressable(harness)
        .into_iter()
        .map(|control| (control.role, control.name))
        .collect();
    let lists = pressable(harness)
        .into_iter()
        .filter(|control| control.role == Role::ComboBox);
    for Control {
        role, name, place, ..
    } in lists.collect::<Vec<_>>()
    {
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

/// After the start-up, the only `CheckHealth` is the one the app sends by itself after a search
/// that worked, so an ask came before each one, and after the one before it.
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

/// A scene that opens with a check may start an ingest after it; any other scene has no file to
/// check, because a test picks none.
fn assert_ingest_commands_follow_a_check(scene: &str, seen: &Seen) {
    let opens_with_a_check = Fake::scene(scene, &testkit::samples_folder())
        .unwrap_or_else(|error| panic!("scene `{scene}`: {error}"))
        .opening()
        .iter()
        .any(|intent| matches!(intent, Intent::CheckIngest(_)));
    let mut checked = false;
    for command in seen.all() {
        match command {
            Command::Preflight { .. } => {
                assert!(
                    opens_with_a_check,
                    "`{scene}` checked a chapter with no file"
                );
                checked = true;
            }
            Command::Ingest { .. } => assert!(
                checked,
                "`{scene}` started an ingest that no check came before"
            ),
            _ => {}
        }
    }
}

/// Every failure that the library state holds: the last failed change of each document, and the
/// last failed delete of a media.
fn library_failures(shared: &gui::state::Shared) -> Vec<&Failure> {
    let media = match &shared.library.media_delete {
        MediaDelete::Failed { failure, .. } => Some(failure),
        MediaDelete::Idle | MediaDelete::Deleting { .. } => None,
    };
    shared.library.failures.values().chain(media).collect()
}

fn assert_nothing_unbuilt_was_reached(harness: &Window, scene: &str, seen: &Seen) {
    assert_ingest_commands_follow_a_check(scene, seen);
    let shared = shared(harness);
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
    let all_failures = failures(shared)
        .into_iter()
        .chain(library_failures(shared))
        .chain(health_failures);
    for failure in all_failures {
        assert_ne!(
            failure.hint, not_built,
            "`{scene}` shows a part that is not built"
        );
    }
    assert_no_unbuilt_name_is_drawn(harness, scene);
    for name in ["Ask", "Library", "Ingest"] {
        assert!(
            harness
                .query_all_by_role_and_label(Role::Tab, name)
                .next()
                .is_some(),
            "`{scene}` has no tab named `{name}`"
        );
    }
}

/// Checked on the screen that is open, so it is called again after each tab is opened.
fn assert_no_unbuilt_name_is_drawn(harness: &Window, scene: &str) {
    assert!(
        harness
            .query_all_by_label_contains("not built")
            .next()
            .is_none(),
        "`{scene}` says that something is not built"
    );
    // Only the Library tab has a Delete control.
    assert!(
        is_open_tab(harness, "Library")
            || harness
                .query_all_by_label_contains("Delete")
                .next()
                .is_none(),
        "`{scene}` draws a Delete control outside the Library tab"
    );
    for name in ABSENT_NAMES {
        assert!(
            harness.query_all_by_label(name).next().is_none(),
            "`{scene}` has a node named `{name}`"
        );
    }
}

/// Whether the library of the scene held a media of each of two titles with a document, when
/// the scene opened. A delete of a document and a delete of a media can both be confirmed then.
fn lists_two_media_with_a_document(harness: &Window) -> bool {
    let catalogue = shared(harness).library.catalogue.ready();
    catalogue.is_some_and(|catalogue| {
        let with_a_document = catalogue
            .media
            .iter()
            .filter(|media| media.title.is_some() && !media.documents.is_empty());
        with_a_document.count() >= 2
    })
}

/// Each sheet has its own confirm, and each control of a scene is pressed once, so a scene that
/// can delete both ends with exactly one delete of each. That shows that the Delete controls were
/// pressed and confirmed, and that no delete answered that it is not built.
fn assert_each_delete_was_confirmed_once(scene: &str, seen: &Seen) {
    let documents = seen.count(|command| matches!(command, Command::DeleteDocument { .. }));
    let media = seen.count(|command| matches!(command, Command::DeleteMedia { .. }));
    assert_eq!(documents, 1, "`{scene}` deleted {documents} documents");
    assert_eq!(media, 1, "`{scene}` deleted {media} media");
}

fn every_key_and_control_of(scene: &str) {
    let (mut harness, seen) = recording::open(scene, DEFAULT_WINDOW);
    testkit::settle(&mut harness);
    assert_start_up_sent_only_its_own_commands(scene, &seen);
    let can_delete_both = lists_two_media_with_a_document(&harness);

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
    for tab in ["Ask", "Library", "Ingest"] {
        click_if_still_there(&mut harness, Role::Tab, tab, 0);
        assert!(
            is_open_tab(&harness, tab),
            "`{scene}` did not open the tab `{tab}`, so its names were not checked"
        );
        assert_no_unbuilt_name_is_drawn(&harness, scene);
        press_every_row_of_every_list(&mut harness);
    }
    assert_nothing_unbuilt_was_reached(&harness, scene, &seen);
    assert_each_health_check_follows_an_ask(scene, &seen);
    if can_delete_both {
        assert_each_delete_was_confirmed_once(scene, &seen);
    }
}

#[test]
fn nothing_in_the_app_leads_to_a_part_that_is_not_built() {
    for scene in fake::scenes().iter().filter(|scene| scene.rests) {
        every_key_and_control_of(scene.name);
    }
}
