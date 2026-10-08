//! A chapter is checked and started from the Ingest page and runs to its end on the fake backend,
//! the catalogue is loaded again when it is stored, and a start that fails says so with its hint.

use eframe::egui;
use eframe::egui::accesskit::Role;
use gui::app::layout::{DEFAULT_WINDOW, MIN_WINDOW};
use gui::contract::{Command, Tab};
use gui::state::IngestJob;
use gui::testkit;

use super::recording::{self, Seen};
use super::{Window, click, has, node, press, says, shared};

const COMMAND: egui::Modifiers = egui::Modifiers::COMMAND;

fn is_preflight(command: &Command) -> bool {
    matches!(command, Command::Preflight { .. })
}

fn is_ingest(command: &Command) -> bool {
    matches!(command, Command::Ingest { .. })
}

fn open(scene: &str, size: [f32; 2]) -> (Window, Seen) {
    let (mut harness, seen) = recording::open(scene, size);
    testkit::settle(&mut harness);
    (harness, seen)
}

fn field(harness: &Window, name: &str) -> Option<String> {
    node(harness, Role::TextInput, name).value()
}

fn place_of(seen: &Seen, is: fn(&Command) -> bool) -> Option<usize> {
    seen.all().iter().position(is)
}

fn a_checked_chapter_runs_to_done_and_loads_the_catalogue_again() {
    let (mut harness, seen) = open("ingest-ready", DEFAULT_WINDOW);
    assert_eq!(shared(&harness).tab, Tab::Ingest);
    let book = node(&harness, Role::ComboBox, "Book").value();
    assert_eq!(book.as_deref(), Some("Option Volatility and Pricing"));
    assert!(
        !has(&harness, Role::TextInput, "Book title"),
        "a book of the library is chosen from the list, not typed"
    );
    let form = ["Sheldon Natenberg", "options, volatility"];
    for (name, text) in ["Author", "Tags"].into_iter().zip(form) {
        assert_eq!(field(&harness, name).as_deref(), Some(text), "{name}");
    }
    assert!(says(&harness, "Chapter 3 · Greeks"));
    assert!(has(&harness, Role::Button, "Start ingest"));
    testkit::save_png(&mut harness, "app-ingest-ready");

    click(&mut harness, Role::Button, "Start ingest");
    let IngestJob::Finished { result, .. } = &shared(&harness).ingest else {
        panic!("the ingest did not finish: {:?}", shared(&harness).ingest);
    };
    assert!(result.is_ok(), "{result:?}");
    assert!(says(
        &harness,
        "Ingested Option Volatility and Pricing, chapter 3: Greeks"
    ));
    assert!(has(&harness, Role::Button, "Add another chapter"));
    testkit::save_png(&mut harness, "app-ingest-done");

    assert_eq!(seen.count(is_preflight), 1);
    assert_eq!(seen.count(is_ingest), 1);
    assert!(place_of(&seen, is_preflight) < place_of(&seen, is_ingest));
    assert_eq!(
        seen.count(|command| matches!(command, Command::LoadCatalogue { .. })),
        2,
        "the catalogue is loaded at the start and again when the chapter is stored"
    );
}

fn a_start_that_fails_says_so_and_try_again_checks_again() {
    let (mut harness, seen) = open("ingest-failed", DEFAULT_WINDOW);
    click(&mut harness, Role::Button, "Start ingest");
    let IngestJob::Finished {
        result: Err(failure),
        ..
    } = &shared(&harness).ingest
    else {
        panic!("the ingest did not fail: {:?}", shared(&harness).ingest);
    };
    let hint = failure.hint.clone();
    assert!(says(&harness, "The ingest failed"));
    assert!(says(&harness, &hint));
    testkit::save_png(&mut harness, "app-ingest-failed");
    assert_eq!(
        seen.count(|command| matches!(command, Command::LoadCatalogue { .. })),
        1,
        "a failed ingest stored nothing, so the catalogue stays as it is"
    );

    click(&mut harness, Role::Button, "Try again");
    assert_eq!(seen.count(is_preflight), 2);
    assert!(
        matches!(shared(&harness).ingest, IngestJob::Checked { .. }),
        "the second check came back"
    );
}

fn the_tab_and_the_keys_open_the_page_and_go_back() {
    let (mut harness, seen) = open("idle", DEFAULT_WINDOW);
    assert_eq!(shared(&harness).tab, Tab::Ask);
    click(&mut harness, Role::Tab, "Ingest");
    assert_eq!(shared(&harness).tab, Tab::Ingest);
    assert!(says(&harness, "Add a chapter"));
    testkit::save_png(&mut harness, "app-idle-tabs");

    press(&mut harness, COMMAND, egui::Key::Num1);
    assert_eq!(shared(&harness).tab, Tab::Ask);
    press(&mut harness, COMMAND, egui::Key::Num3);
    assert_eq!(shared(&harness).tab, Tab::Ingest);
    press(&mut harness, COMMAND, egui::Key::Num1);
    assert_eq!(shared(&harness).tab, Tab::Ask);
    assert!(
        seen.all()
            .iter()
            .all(|command| !is_preflight(command) && !is_ingest(command)),
        "opening the page checks nothing"
    );
}

fn the_page_fits_the_smallest_window() {
    let (mut harness, _) = open("ingest-ready", MIN_WINDOW);
    assert!(has(&harness, Role::Button, "Start ingest"));
    testkit::save_png(&mut harness, "app-ingest-ready-min");
}

#[test]
fn a_checked_chapter_runs_to_done_on_the_fake_and_the_catalogue_is_loaded_again() {
    a_checked_chapter_runs_to_done_and_loads_the_catalogue_again();
    a_start_that_fails_says_so_and_try_again_checks_again();
    the_tab_and_the_keys_open_the_page_and_go_back();
    the_page_fits_the_smallest_window();
}
