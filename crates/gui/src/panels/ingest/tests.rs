//! Draws the Ingest page in a test window and checks what a person reads, what the buttons
//! send and when the page asks for a new check.

use std::path::PathBuf;

use eframe::egui;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

use super::{Local, show};
use crate::contract::{
    ChapterState, Failure, FailureKind, IngestOutcome, IngestProgress, IngestReport, IngestRequest,
    IngestStage, Intent, Loadable, PageToCheck, Preflight, RequestId,
};
use crate::state::{IngestJob, Shared};
use crate::testkit::{self, Host, sample};

/// The page area of the smallest window.
const SMALLEST: [f32; 2] = [1148.0, 644.0];

fn page(shared: Shared) -> Harness<'static, Host> {
    let mut local = Local::default();
    testkit::panel(SMALLEST, shared, move |ui, cx| show(ui, &mut local, cx))
}

fn request() -> IngestRequest {
    IngestRequest {
        pdf: PathBuf::from("/books/option-volatility/chapter-1-sample-pages.pdf"),
        book: "Option Volatility and Pricing".to_owned(),
        author: Some("Sheldon Natenberg".to_owned()),
        tags: vec!["options".to_owned(), "volatility".to_owned()],
    }
}

fn shared_with(ingest: IngestJob) -> Shared {
    let mut shared = Shared::default();
    shared.ingest = ingest;
    shared
}

fn checked(preflight: Preflight) -> IngestJob {
    IngestJob::Checked {
        request: request(),
        preflight,
    }
}

fn running(stage: Option<IngestStage>) -> IngestJob {
    IngestJob::Running {
        request: request(),
        id: RequestId(7),
        progress: stage.map(|stage| IngestProgress {
            stage,
            done: None,
            total: None,
            pages_failed: 0,
            cost_usd: 0.0,
        }),
    }
}

fn finished(result: Result<IngestOutcome, Failure>) -> IngestJob {
    IngestJob::Finished {
        request: request(),
        result,
    }
}

fn set(harness: &mut Harness<'_, Host>, ingest: IngestJob) {
    harness.state_mut().shared.ingest = ingest;
    harness.run();
}

fn says(harness: &Harness<'_, Host>, words: &str) -> bool {
    harness.query_all_by_label_contains(words).next().is_some()
}

fn has_button(harness: &Harness<'_, Host>, name: &str) -> bool {
    harness
        .query_by_role_and_label(Role::Button, name)
        .is_some()
}

fn is_disabled(harness: &Harness<'_, Host>, role: Role, name: &str) -> bool {
    harness
        .get_by_role_and_label(role, name)
        .accesskit_node()
        .is_disabled()
}

/// Runs frames as the app does: the intents of each frame are applied before the next one.
/// Returns every intent that the frames pushed, in order.
fn frames(harness: &mut Harness<'_, Host>) -> Vec<Intent> {
    let mut sent = Vec::new();
    for _ in 0..4 {
        harness.step();
        sent.extend(harness.state().intents.clone());
        harness.state_mut().apply_intents();
    }
    sent
}

fn press(harness: &mut Harness<'_, Host>, name: &str) -> Vec<Intent> {
    harness.get_by_role_and_label(Role::Button, name).click();
    frames(harness)
}

fn type_into(harness: &mut Harness<'_, Host>, field: &str, text: &str) -> Vec<Intent> {
    harness
        .get_by_role_and_label(Role::TextInput, field)
        .click();
    harness.run();
    harness
        .get_by_role_and_label(Role::TextInput, field)
        .type_text(text);
    frames(harness)
}

#[test]
fn an_empty_form_shows_the_rule_and_the_cost_and_checks_only_a_whole_form() {
    let mut shared = Shared::default();
    shared.library.catalogue = Loadable::Ready(sample::catalogue());
    let mut harness = page(shared);
    harness.run();
    assert!(says(
        &harness,
        "The file must be named chapter-<number>-<name>.pdf, for example chapter-3-greeks.pdf."
    ));
    assert!(says(
        &harness,
        "Checking is free. Starting is paid work: claude and Jev convert each page, Gemini embeds the items, and claude reads the concepts."
    ));
    assert!(says(&harness, "No file chosen"));
    testkit::save_png(&mut harness, "ingest-empty");

    assert_eq!(press(&mut harness, "Choose a PDF"), vec![Intent::PickPdf]);

    // Nothing is checked with no file, and nothing with a file and no book.
    assert!(is_disabled(&harness, Role::Button, "Check the chapter"));
    assert!(press(&mut harness, "Check the chapter").is_empty());
    let pdf = request().pdf;
    let picked = Intent::PdfPicked(pdf.clone());
    harness
        .state_mut()
        .shared
        .apply_intent(picked, &mut Vec::new());
    harness.run();
    assert!(says(&harness, "chapter-1-sample-pages.pdf"));
    assert!(is_disabled(&harness, Role::Button, "Check the chapter"));
    assert!(press(&mut harness, "Check the chapter").is_empty());

    // A book of the library fills the title; typing over it changes it.
    harness.get_by_label("Books in the library").click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Quanty Sample Notes")
        .click();
    harness.run();
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Book title")
            .value()
            .as_deref(),
        Some("Quanty Sample Notes")
    );
    harness
        .get_by_role_and_label(Role::TextInput, "Book title")
        .click();
    harness.run();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness
        .get_by_role_and_label(Role::TextInput, "Book title")
        .type_text("  Option Volatility and Pricing ");
    frames(&mut harness);
    type_into(&mut harness, "Tags", "options, , volatility ,");
    testkit::save_png(&mut harness, "ingest-filled");

    let wanted = IngestRequest {
        author: None,
        ..IngestRequest { pdf, ..request() }
    };
    assert_eq!(
        press(&mut harness, "Check the chapter"),
        vec![Intent::CheckIngest(wanted)]
    );
}

#[test]
fn a_checked_chapter_can_be_started_and_a_blocked_or_refused_one_cannot() {
    let new = sample::preflight();
    let mut harness = page(shared_with(checked(new.clone())));
    harness.run();
    assert!(says(&harness, "Chapter 1 · Sample Pages"));
    assert!(says(&harness, "Not converted yet."));
    testkit::save_png(&mut harness, "ingest-ready");
    assert_eq!(
        press(&mut harness, "Start ingest"),
        vec![Intent::StartIngest]
    );

    for (state, words) in [
        (
            ChapterState::PartlyConverted { pages_done: 3 },
            "3 of 7 pages are converted already and are not paid for again.",
        ),
        (
            ChapterState::Converted,
            "All 7 pages are converted already: only the embedding and the concepts are paid for.",
        ),
        (
            ChapterState::Ingested { items: 12 },
            "Already ingested with 12 items: a start converts nothing and pays for nothing.",
        ),
    ] {
        let preflight = Preflight {
            state: Some(state),
            ..new.clone()
        };
        set(&mut harness, checked(preflight));
        assert!(says(&harness, words), "{words}");
        assert!(has_button(&harness, "Start ingest"));
    }

    let key = sample::failure(FailureKind::ConverterKeyMissing);
    let blocked = Preflight {
        blockers: vec![key.clone()],
        ..new
    };
    set(&mut harness, checked(blocked));
    assert!(says(&harness, &key.hint));
    assert!(says(&harness, &key.detail));
    assert!(!has_button(&harness, "Start ingest"));
    assert!(has_button(&harness, "Check the chapter"));
    testkit::save_png(&mut harness, "ingest-blocked");

    let refused = sample::failure(FailureKind::BadFile);
    let failed = IngestJob::CheckFailed {
        request: request(),
        failure: refused.clone(),
    };
    set(&mut harness, failed);
    assert!(says(&harness, "The chapter cannot be ingested yet"));
    assert!(says(&harness, &refused.hint));
    assert!(!has_button(&harness, "Start ingest"));
    testkit::save_png(&mut harness, "ingest-check-failed");
}

#[test]
fn a_running_ingest_shows_its_stage_and_takes_no_input() {
    let mut harness = page(shared_with(running(None)));
    harness.run();
    let stages = [
        (None, "Starting: reading the PDF and asking the stores"),
        (Some(IngestStage::PreparingPages), "Converting the pages"),
        (Some(IngestStage::Converting), "Converting the pages"),
        (
            Some(IngestStage::WritingGraph),
            "Storing the items and reading the concepts",
        ),
        (
            Some(IngestStage::LinkingConcepts),
            "Storing the items and reading the concepts",
        ),
    ];
    for (stage, words) in stages {
        set(&mut harness, running(stage));
        harness.get_by_role_and_label(Role::ProgressIndicator, words);
        assert!(says(&harness, "Keep the app open until this is done."));
    }
    let stopping = IngestJob::Stopping {
        request: request(),
        id: RequestId(7),
        progress: None,
    };
    set(&mut harness, stopping);
    harness.get_by_role_and_label(
        Role::ProgressIndicator,
        "Starting: reading the PDF and asking the stores",
    );

    set(&mut harness, running(Some(IngestStage::Converting)));
    for name in ["Start ingest", "Cancel", "Check the chapter", "Try again"] {
        assert!(!has_button(&harness, name), "{name}");
    }
    for field in ["Book title", "Author", "Tags"] {
        assert!(is_disabled(&harness, Role::TextInput, field), "{field}");
    }
    assert!(is_disabled(&harness, Role::Button, "Choose a PDF"));
    testkit::save_png(&mut harness, "ingest-running");
}

#[test]
fn a_finished_ingest_shows_its_summary_or_its_failure_with_the_hint() {
    let report = sample::ingest_report();
    let done = Ok(IngestOutcome::Ingested(report.clone()));
    let mut harness = page(shared_with(finished(done)));
    harness.run();
    assert!(says(&harness, &format!("Ingested {}", report.title)));
    assert!(says(
        &harness,
        "7 pages, 12 passages, 3 formulas, 2 figures and 1 table. 14 concepts created, 9 linked. The conversion cost $0.42. 1 page needs a check."
    ));
    testkit::save_png(&mut harness, "ingest-done");

    let more = sample::ingest_report();
    let busy = IngestReport {
        cost_usd: None,
        skipped_items: 2,
        pages_to_check: vec![PageToCheck::default(); 3],
        ..more
    };
    set(&mut harness, finished(Ok(IngestOutcome::Ingested(busy))));
    assert!(says(
        &harness,
        "9 linked. 3 pages need a check. 2 items were skipped."
    ));
    assert!(!says(&harness, "cost"));

    let already = IngestOutcome::AlreadyIngested {
        doc: report.doc,
        items: 12,
        pages_to_check: None,
    };
    set(&mut harness, finished(Ok(already)));
    assert!(says(&harness, "Already ingested"));
    assert!(says(
        &harness,
        "12 items are stored. Nothing was converted or embedded."
    ));
    assert!(!has_button(&harness, "Try again"));
    testkit::save_png(&mut harness, "ingest-already");

    set(&mut harness, finished(Ok(IngestOutcome::Cancelled)));
    assert!(says(&harness, "Start the same PDF again to go on."));

    let failure = sample::failure(FailureKind::PageFailed);
    set(&mut harness, finished(Err(failure.clone())));
    assert!(says(&harness, "The ingest failed"));
    assert!(says(&harness, &failure.hint));
    assert!(says(
        &harness,
        "Pages that are converted are kept and are not paid for twice."
    ));
    testkit::save_png(&mut harness, "ingest-failed");
    assert_eq!(
        press(&mut harness, "Try again"),
        vec![Intent::CheckIngest(request())]
    );

    // Another chapter clears the job and the file, and keeps the book.
    set(&mut harness, finished(Err(failure)));
    assert_eq!(
        press(&mut harness, "Add another chapter"),
        vec![Intent::ClearIngest]
    );
    assert_eq!(harness.state().shared.ingest, IngestJob::Idle);
    assert!(says(&harness, "No file chosen"));
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Book title")
            .value()
            .as_deref(),
        Some("Option Volatility and Pricing")
    );
}

#[test]
fn a_form_changed_after_its_check_asks_for_a_new_check() {
    let mut harness = page(shared_with(checked(sample::preflight())));
    harness.run();
    assert!(
        harness.state().intents.is_empty(),
        "a form filled from its own check is not a change"
    );

    assert_eq!(
        type_into(&mut harness, "Author", "s"),
        vec![Intent::ClearIngest]
    );
    assert_eq!(harness.state().shared.ingest, IngestJob::Idle);

    harness.state_mut().shared.ingest = running(Some(IngestStage::Converting));
    assert!(
        frames(&mut harness).is_empty(),
        "a running ingest is never cleared by the form, which is now another one"
    );
}
