//! A checked chapter is started and runs to its end on the fake backend, the catalogue is loaded
//! again when it is stored, and a start that fails says so with its hint. The check and the run
//! show how far they have got, and each stage of a run is named with its counts and its cost.

use std::path::PathBuf;

use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use gui::app::layout::{DEFAULT_WINDOW, MIN_WINDOW};
use gui::backend::fake::Fake;
use gui::backend::{Handler, Reply};
use gui::contract::{
    Category, Command, DocumentName, IngestProgress, IngestRequest, IngestStage, ModelTokens,
    RequestId, Tab, Usage,
};
use gui::panels::ingest;
use gui::state::{IngestJob, Shared};
use gui::testkit::{self, run_until};

use super::{is_ingest, is_preflight, last_checked, open, place_of};
use crate::flows::recording;
use crate::flows::{COMMAND, click, field, has, is_enabled, node, press, says, shared, type_into};

const CHOSEN_BOOK: &str = "Option Volatility and Pricing · Book";

fn a_checked_chapter_runs_to_done_and_loads_the_catalogue_again() {
    let (mut harness, seen) = open("ingest-ready", DEFAULT_WINDOW);
    assert_eq!(shared(&harness).tab, Tab::Ingest);
    let media = node(&harness, Role::ComboBox, "Media").value();
    assert_eq!(media.as_deref(), Some(CHOSEN_BOOK));
    assert!(
        !has(&harness, Role::TextInput, "Media title"),
        "a media of the library is chosen from the list, not typed"
    );
    for name in ["Authors", "Tags"] {
        assert!(
            !has(&harness, Role::TextInput, name),
            "the labels of a chosen media are fixed on its card: `{name}` is a box"
        );
    }
    assert!(
        has(&harness, Role::Label, "Book"),
        "the card names the category"
    );
    assert!(has(
        &harness,
        Role::Button,
        "Edit Option Volatility and Pricing"
    ));
    assert!(
        !has(&harness, Role::Button, "Check the chapter"),
        "a check needs no button"
    );
    assert_eq!(
        field(&harness, "Tags for this PDF").as_deref(),
        Some("greeks")
    );
    assert!(says(
        &harness,
        "Chapter 3 · Greeks — 12 pages. Not converted yet."
    ));
    assert!(has(&harness, Role::Button, "Start ingest"));
    testkit::save_png(&mut harness, "app-ingest-ready");

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
            if progress.stage == IngestStage::Converting && progress.done == Some(page)
    )
}

fn a_run_shows_its_page_and_its_cost() {
    let (mut harness, _) = open("ingest-running", DEFAULT_WINDOW);
    // The ingest never ends, so the app is never idle, and `click` would wait for ever.
    node(&harness, Role::Button, "Start ingest").click();
    run_until(&mut harness, "page 3", |shared| is_at_page(shared, 3));
    harness.run_ok();

    let words = "Converting the pages — page 3 of 12";
    let bar = node(&harness, Role::ProgressIndicator, words);
    assert_eq!(bar.accesskit_node().numeric_value(), Some(0.25));
    assert!(says(&harness, "48k tokens · ≈ $0.42 so far"));
    assert!(
        !is_enabled(&harness, Role::Button, "Choose a PDF"),
        "the form is off while an ingest runs"
    );
}

#[test]
fn the_check_and_the_run_show_their_progress() {
    a_check_shows_a_bar_with_no_count();
    a_run_shows_its_page_and_its_cost();
}

/// What the run used after `pages` pages: Sonnet reads 12,000 tokens and writes 4,000 for each
/// page, at $0.14 a page.
fn progress(stage: IngestStage, counts: (Option<u32>, Option<u32>), pages: u32) -> IngestProgress {
    let pages_read = u64::from(pages);
    let sonnet = ModelTokens {
        model: "claude-sonnet-5-5".to_owned(),
        input: 12_000 * pages_read,
        output: 4_000 * pages_read,
        ..ModelTokens::default()
    };
    IngestProgress {
        stage,
        done: counts.0,
        total: counts.1,
        pages_failed: 0,
        spent: Usage {
            models: vec![sonnet],
            cost_usd: Some(0.14 * f64::from(pages)),
        },
    }
}

/// How one stage is shown: its words, the share its bar shows when it counts, and its cost line.
struct Shown {
    progress: Option<IngestProgress>,
    words: &'static str,
    share: Option<f64>,
    cost: Option<&'static str>,
}

fn every_stage() -> [Shown; 8] {
    let spent = Some("192k tokens · ≈ $1.68 so far");
    let shown = |progress, words, share, cost| Shown {
        progress,
        words,
        share,
        cost,
    };
    [
        shown(None, "Reading the PDF", None, None),
        shown(
            Some(progress(IngestStage::PreparingPages, (None, Some(12)), 0)),
            "Reading the PDF",
            None,
            None,
        ),
        shown(
            Some(progress(IngestStage::Converting, (Some(3), Some(12)), 3)),
            "Converting the pages — page 3 of 12",
            Some(0.25),
            Some("48k tokens · ≈ $0.42 so far"),
        ),
        shown(
            Some(progress(IngestStage::WritingGraph, (None, None), 12)),
            "Writing the graph",
            None,
            spent,
        ),
        shown(
            Some(progress(IngestStage::Embedding, (None, Some(120)), 12)),
            "Embedding 120 items",
            None,
            spent,
        ),
        shown(
            Some(progress(IngestStage::Storing, (None, None), 12)),
            "Storing the items",
            None,
            spent,
        ),
        shown(
            Some(progress(
                IngestStage::ReadingConcepts,
                (Some(40), Some(120)),
                12,
            )),
            "Reading the concepts — 40 of 120",
            Some(40.0 / 120.0),
            spent,
        ),
        shown(
            Some(progress(
                IngestStage::LinkingConcepts,
                (Some(10), Some(120)),
                12,
            )),
            "Linking the concepts — 10 of 120",
            Some(10.0 / 120.0),
            spent,
        ),
    ]
}

#[test]
fn every_stage_of_a_run_is_named_with_its_counts() {
    let request = IngestRequest {
        pdf: PathBuf::from("chapter-3-greeks.pdf"),
        media: "Option Volatility and Pricing".to_owned(),
        category: Category::Book,
        name: DocumentName::Chapter {
            number: 3,
            name: "Greeks".to_owned(),
        },
        tags: Vec::new(),
    };
    let mut local = ingest::Local::default();
    let mut harness = testkit::panel([720.0, 600.0], Shared::default(), move |ui, cx| {
        ingest::show(ui, &mut local, cx);
    });

    for stage in every_stage() {
        harness.state_mut().shared.ingest = IngestJob::Running {
            request: request.clone(),
            id: RequestId(1),
            progress: stage.progress,
        };
        harness.run_ok();

        let words = stage.words;
        let bar = harness
            .query_all_by_role_and_label(Role::ProgressIndicator, words)
            .next()
            .unwrap_or_else(|| panic!("no bar is named `{words}`"));
        let share = bar.accesskit_node().numeric_value();
        let is_share = match (share, stage.share) {
            (Some(shown), Some(wanted)) => (shown - wanted).abs() < 1e-6,
            (shown, wanted) => shown == wanted,
        };
        assert!(
            is_share,
            "`{words}`: the bar shows {share:?}, not {:?}",
            stage.share
        );
        assert!(
            harness
                .query_all_by_role_and_label(Role::Label, words)
                .next()
                .is_some(),
            "`{words}` is not said"
        );
        let is_cost_said = match stage.cost {
            Some(cost) => harness.query_all_by_label(cost).next().is_some(),
            None => harness
                .query_all_by_label_contains("so far")
                .next()
                .is_none(),
        };
        assert!(is_cost_said, "`{words}` should cost {:?}", stage.cost);
    }
}
