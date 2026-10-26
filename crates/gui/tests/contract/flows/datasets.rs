//! The Datasets page on the whole app and the fake backend: a build starts from the form and ends
//! in a notice, shows its stage and its cost while it runs, and a build that failed shows it.

use std::time::{Duration, Instant};

use backend::Runtime;
use backend::fake::Fake;
use contract::{Command, DatasetRequest, DatasetStage, DocId, Event, FailureKind, RequestId, Tab};
use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::NodeT as _;
use gui::app::layout::DEFAULT_WINDOW;
use gui::state::{DatasetJob, Shared};
use gui::testkit::{self, run_until};
use uuid::Uuid;

use crate::flows::recording::{self, Seen};
use crate::flows::{
    COMMAND, Window, click, field, has, is_enabled, node, press, says, shared, type_into,
};

/// The stage at which the scenes with a build that never ends stand still.
const STALL: DatasetStage = DatasetStage::WritingSynthetic { done: 6, total: 12 };

fn open(scene: &str, size: [f32; 2]) -> (Window, Seen) {
    let (mut harness, seen) = recording::open(scene, size);
    testkit::settle(&mut harness);
    (harness, seen)
}

/// The builds the app sent, in the order it sent them.
fn sent_builds(seen: &Seen) -> Vec<DatasetRequest> {
    seen.all()
        .into_iter()
        .filter_map(|command| match command {
            Command::BuildDataset { build, .. } => Some(build),
            _ => None,
        })
        .collect()
}

/// Whether a build that never ends has played up to its stall.
fn is_at_its_stall(shared: &Shared) -> bool {
    matches!(
        &shared.datasets,
        DatasetJob::Running { progress: Some(progress), .. } if progress.stage == STALL
    )
}

const DRY_RUN: &str = "Dry run (export, check and plan only; asks no model)";
const CHAPTER_2: &str = "Chapter 2 · Black Scholes In Depth";
const LIMIT: Duration = Duration::from_secs(5);

fn the_keys_and_the_strip_open_the_page() {
    let (mut harness, _) = open("datasets-ready", DEFAULT_WINDOW);
    assert_eq!(shared(&harness).tab, Tab::Datasets);

    press(&mut harness, COMMAND, egui::Key::Num1);
    assert_eq!(shared(&harness).tab, Tab::Ask);
    press(&mut harness, COMMAND, egui::Key::Num4);
    assert_eq!(shared(&harness).tab, Tab::Datasets);

    click(&mut harness, Role::Tab, "Ask");
    assert_eq!(shared(&harness).tab, Tab::Ask);
    click(&mut harness, Role::Tab, "Datasets");
    assert_eq!(shared(&harness).tab, Tab::Datasets);
    assert!(says(&harness, "Build a training dataset"));
}

fn fill_the_form(harness: &mut super::Window) {
    type_into(harness, "Dataset name", "notes-v2");
    click(harness, Role::ComboBox, "Media");
    click(harness, Role::Button, "Quanty Sample Notes");
    click(harness, Role::ComboBox, "Hold out");
    click(harness, Role::CheckBox, CHAPTER_2);
    press(harness, egui::Modifiers::NONE, egui::Key::Escape);
    click(harness, Role::Button, "More rewrites per section");
}

fn a_build_of_the_form_runs_to_a_done_notice() {
    let (mut harness, seen) = open("datasets-ready", DEFAULT_WINDOW);
    assert!(
        !is_enabled(&harness, Role::Button, "Start build"),
        "a build needs a name"
    );
    fill_the_form(&mut harness);
    click(&mut harness, Role::Button, "Start build");

    let wanted = DatasetRequest {
        name: "notes-v2".to_owned(),
        media: Some("Quanty Sample Notes".to_owned()),
        holdout: vec![DocId(Uuid::from_u128(2))],
        rewrites: 4,
        questions: 2,
        max_sft: 300,
        dry_run: false,
    };
    assert_eq!(sent_builds(&seen), [wanted]);
    let DatasetJob::Finished { result, .. } = &shared(&harness).datasets else {
        panic!("the build did not finish: {:?}", shared(&harness).datasets);
    };
    assert!(result.is_ok(), "{result:?}");
    assert!(says(&harness, "Built notes-v2"));
    assert!(says(
        &harness,
        "2 documents, 4 sections for training and 2 held out, 1 rejected. 12 synthetic texts and 8 fine-tuning examples (5 think, 3 direct)."
    ));
    assert!(says(&harness, "Saved in datasets/notes-v2"));
    assert_eq!(shared(&harness).notices[0].title, "Built notes-v2");
    testkit::save_png(&mut harness, "app-datasets-done");

    click(&mut harness, Role::Button, "Build another");
    assert_eq!(shared(&harness).datasets, DatasetJob::Idle);
    assert!(is_enabled(&harness, Role::Button, "Start build"));
}

fn a_dry_run_tells_its_plan() {
    let (mut harness, seen) = open("datasets-ready", DEFAULT_WINDOW);
    type_into(&mut harness, "Dataset name", "plan-only");
    click(&mut harness, Role::CheckBox, DRY_RUN);
    click(&mut harness, Role::Button, "Start build");

    let builds = sent_builds(&seen);
    assert_eq!(builds.len(), 1);
    assert!(builds[0].dry_run);
    assert!(says(
        &harness,
        "Dry run: 70 model calls planned, none made."
    ));
}

fn next_event(backend: &mut Runtime<impl backend::Handler>) -> Event {
    let started = Instant::now();
    loop {
        if let Some(event) = backend.try_next() {
            return event;
        }
        assert!(started.elapsed() < LIMIT, "the next event never came");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// The name of a stage, whatever its counts.
fn kind_of(stage: DatasetStage) -> &'static str {
    match stage {
        DatasetStage::Exporting { .. } => "exporting",
        DatasetStage::Checking => "checking",
        DatasetStage::Planning => "planning",
        DatasetStage::WritingSynthetic { .. } => "synthetic",
        DatasetStage::WritingSft { .. } => "sft",
        DatasetStage::WritingManifest => "manifest",
    }
}

fn the_fake_plays_every_stage_in_order_and_ends_once() {
    let fake = Fake::scene("datasets-ready", &testkit::samples_folder())
        .expect("the scene starts")
        .instant();
    let mut backend = Runtime::start_own_threads(fake, || {}).expect("the runtime starts");
    let build = DatasetRequest {
        name: "all-stages".to_owned(),
        ..DatasetRequest::default()
    };
    backend.send(Command::BuildDataset {
        request: RequestId(1),
        build,
    });

    let mut kinds: Vec<&str> = Vec::new();
    let mut spent = 0;
    loop {
        match next_event(&mut backend) {
            Event::DatasetProgress { request, progress } => {
                assert_eq!(request, RequestId(1));
                let kind = kind_of(progress.stage);
                if kinds.last() != Some(&kind) {
                    kinds.push(kind);
                }
                assert!(progress.spent.tokens() >= spent, "the spend fell");
                spent = progress.spent.tokens();
            }
            Event::DatasetFinished { request, result } => {
                assert_eq!(request, RequestId(1));
                assert!(result.is_ok(), "{result:?}");
                break;
            }
            other => panic!("a build sent {other:?}"),
        }
    }
    let every_stage = [
        "exporting",
        "checking",
        "planning",
        "synthetic",
        "sft",
        "manifest",
    ];
    assert_eq!(kinds, every_stage);
    let started = Instant::now();
    while !backend.is_idle() {
        assert!(started.elapsed() < LIMIT, "the build did not end its task");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(backend.try_next().is_none(), "nothing follows the end");
}

#[test]
fn a_build_starts_from_the_form_and_ends_in_a_done_notice() {
    the_keys_and_the_strip_open_the_page();
    a_build_of_the_form_runs_to_a_done_notice();
    a_dry_run_tells_its_plan();
    the_fake_plays_every_stage_in_order_and_ends_once();
}

fn a_running_build_shows_its_stage_its_cost_and_a_form_that_is_off() {
    let mut harness = testkit::app("datasets-running", DEFAULT_WINDOW);
    // The build never ends, so the app is never idle, and `settle` would wait for ever.
    run_until(&mut harness, "the stall of the build", is_at_its_stall);
    harness.run_ok();

    let words = "Writing the synthetic texts — 6 of 12, about 50%";
    let bar = node(&harness, Role::ProgressIndicator, words);
    assert_eq!(bar.accesskit_node().numeric_value(), Some(0.5));
    assert!(says(&harness, "31k tokens, about $0.12 so far"));
    assert_eq!(
        field(&harness, "Dataset name").as_deref(),
        Some("notes-v1"),
        "the form was filled from the build"
    );
    assert!(
        !is_enabled(&harness, Role::TextInput, "Dataset name"),
        "the form is off while a build runs"
    );
    assert!(!has(&harness, Role::Button, "Start build"));
}

fn a_failed_build_says_why_and_try_again_starts_the_same_build() {
    let (mut harness, seen) = recording::open("datasets-failed", DEFAULT_WINDOW);
    testkit::settle(&mut harness);
    let DatasetJob::Finished {
        result: Err(failure),
        ..
    } = &shared(&harness).datasets
    else {
        panic!("the build did not fail: {:?}", shared(&harness).datasets);
    };
    assert_eq!(failure.kind, FailureKind::ClaudeUsageLimit);
    let hint = failure.hint.clone();
    assert!(says(&harness, "The build failed"));
    assert!(says(&harness, &hint));
    assert_eq!(
        shared(&harness).notices[0].title,
        "Could not build notes-v1"
    );

    click(&mut harness, Role::Button, "Try again");
    let builds = sent_builds(&seen);
    assert_eq!(builds.len(), 2);
    assert_eq!(builds[0], builds[1], "the same build is started again");
    assert_eq!(builds[0].name, "notes-v1");
}

#[test]
fn a_running_build_shows_its_stage_and_a_failed_one_its_hint() {
    a_running_build_shows_its_stage_its_cost_and_a_form_that_is_off();
    a_failed_build_says_why_and_try_again_starts_the_same_build();
}
