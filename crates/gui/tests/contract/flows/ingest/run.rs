//! A checked chapter is started and runs to its end on the fake backend, the catalogue is loaded
//! again when it is stored, and a start that fails says so with its hint. The check and the run
//! show how far they have got.

use backend::fake::Fake;
use backend::{Handler, Reply};
use contract::{Command, IngestStage, Tab};
use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::NodeT as _;
use gui::app::layout::{DEFAULT_WINDOW, MIN_WINDOW};
use gui::state::{IngestJob, Shared};
use gui::testkit::{self, run_until};

use super::{is_ingest, is_preflight, last_checked, open, place_of};
use crate::flows::recording;
use crate::flows::{COMMAND, click, has, is_enabled, node, press, says, shared, type_into};

const CHOSEN_BOOK: &str = "Option Volatility and Pricing (Book)";
/// The share of a page that the bar counts as done once the page is cut out but not yet saved.
const CUT: f64 = 0.15;

fn a_checked_chapter_runs_to_done_and_loads_the_catalogue_again() {
    let (mut harness, seen) = open("ingest-ready", DEFAULT_WINDOW);
    type_into(&mut harness, "Tags for this PDF", ", vega");
    assert!(
        !is_enabled(&harness, Role::Button, "Start ingest"),
        "a typed change that is not checked yet must not start the old request"
    );
    assert_eq!(
        seen.count(is_preflight),
        1,
        "nothing is checked while the box has the keyboard"
    );
    press(&mut harness, egui::Modifiers::NONE, egui::Key::Enter);
    assert_eq!(seen.count(is_preflight), 2);
    assert_eq!(last_checked(&seen).tags, ["greeks", "vega"]);

    // The box keeps the keyboard, and its tags are still the checked ones, so Start must start the
    // checked PDF and send no other check.
    type_into(&mut harness, "Tags for this PDF", ",");
    click(&mut harness, Role::Button, "Start ingest");
    let IngestJob::Finished { result, .. } = &shared(&harness).ingest else {
        panic!("the ingest did not finish: {:?}", shared(&harness).ingest);
    };
    assert!(result.is_ok(), "{result:?}");
    assert!(says(
        &harness,
        "Ingested Option Volatility and Pricing, chapter 3: Greeks"
    ));
    assert!(says(
        &harness,
        "Tokens: claude-haiku-5-5 48k, claude-sonnet-5-5 192k, gemini-embedding-2 9k (estimated). Cost ≈ $1.75."
    ));
    assert!(says(
        &harness,
        "The insights, takeaways and cheat sheet are written: the Library shows them under Insights."
    ));
    assert!(has(&harness, Role::Button, "Add another PDF"));
    testkit::save_png(&mut harness, "app-ingest-done");

    assert_eq!(seen.count(is_preflight), 2);
    assert_eq!(seen.count(is_ingest), 1);
    let last_preflight = seen.all().iter().rposition(is_preflight);
    assert!(last_preflight < place_of(&seen, is_ingest));
    assert_eq!(
        seen.count(|command| matches!(command, Command::LoadCatalogue { .. })),
        2,
        "the catalogue is loaded at the start and again when the chapter is stored"
    );

    click(&mut harness, Role::Button, "Add another PDF");

    let media = node(&harness, Role::ComboBox, "Media").value();
    assert_eq!(
        media.as_deref(),
        Some(CHOSEN_BOOK),
        "the media stays chosen"
    );
    assert!(says(&harness, "No file chosen"));
    assert_eq!(
        seen.count(is_preflight),
        2,
        "a form with no file is not checked"
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
    assert!(says(&harness, "Add media"));
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

/// Answers every command but a check, so a check never ends.
struct PreflightNeverEnds(Fake);

impl Handler for PreflightNeverEnds {
    async fn serve(&self, command: Command, reply: Reply) {
        if !is_preflight(&command) {
            self.0.serve(command, reply).await;
        }
    }
}

fn a_check_shows_a_bar_with_no_count() {
    let (mut harness, _) = recording::open_with("ingest-ready", DEFAULT_WINDOW, PreflightNeverEnds);
    // The check never ends, so the app is never idle, and `settle` would wait for ever.
    run_until(&mut harness, "the catalogue", |shared| {
        shared.library.catalogue.ready().is_some()
    });
    let bar = node(&harness, Role::ProgressIndicator, "Checking the PDF");
    assert_eq!(bar.accesskit_node().numeric_value(), None);
    assert!(!has(&harness, Role::Button, "Start ingest"));
}

fn is_at_page(shared: &Shared, page: u32) -> bool {
    matches!(
        &shared.ingest,
        IngestJob::Running { progress: Some(progress), .. }
            if matches!(progress.stage, IngestStage::Converting { saved, .. } if saved == page)
    )
}

fn a_run_shows_its_page_and_its_cost() {
    let (mut harness, _) = open("ingest-running", DEFAULT_WINDOW);
    // The ingest never ends, so the app is never idle, and `click` would wait for ever.
    node(&harness, Role::Button, "Start ingest").click();
    run_until(&mut harness, "page 3", |shared| is_at_page(shared, 3));
    harness.run_ok();

    let words = "Converting the pages — 3 of 12 done, about 36%";
    let bar = node(&harness, Role::ProgressIndicator, words);
    let share = bar
        .accesskit_node()
        .numeric_value()
        .expect("a bar that counts");
    assert!((share - (3.0 + CUT * 9.0) / 12.0).abs() < 1e-6, "{share}");
    assert!(says(&harness, "48k tokens, about $0.42 so far"));
    assert!(
        !is_enabled(&harness, Role::Button, "Choose a PDF"),
        "the form is off while an ingest runs"
    );
}

#[test]
fn the_check_and_the_run_show_their_progress() {
    a_check_shows_a_bar_with_no_count();
    a_run_shows_its_page_and_its_cost();
    super::insights::assert_artifacts_precede_store_changes();
}
