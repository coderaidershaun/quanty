//! The look of the app at rest is kept as pictures on disk, and a change that moves a pixel by
//! more than the tolerance fails here until a person looks at the new picture and approves it.

use std::time::{Duration, Instant};

use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::Queryable as _;
use egui_kittest::{Harness, SnapshotOptions};
use gui::app::App;
use gui::app::layout::{self, DEFAULT_WINDOW, MIN_WINDOW};
use gui::testkit;

type Window = Harness<'static, App>;

/// A kept picture is one pixel for each point, so that the folder stays small.
const PIXELS_PER_POINT: f32 = 1.0;
const TOLERANCE: f32 = 0.6;
const FOLDER: &str = "tests/snapshots";

/// The chip of the figure's result. Its click is what a person does to open the source.
const FIGURE_CITATION: &str = "Citation 8";
const START_INGEST: &str = "Start ingest";
/// The pencil of the first media of the scene, whose form is on screen with no scroll.
const FIRST_MEDIA_PENCIL: &str = "Edit Hawkes Processes in Finance";
/// The ingest of its scene stops for ever at page 3, so its bar rests there.
const BAR_AT_PAGE_3: &str = "Converting the pages — 3 of 12 done · about 36%";
const LIMIT: Duration = Duration::from_secs(5);

enum Start {
    AsItOpens,
    AfterAClickOnTheFigureCitation,
    AfterAClickOnStartIngest,
    AfterAStartThatNeverEnds,
    AfterANewMediaIsTyped,
    AfterAClickOnTheLibraryTab,
    AfterAClickOnAMediaPencil,
}

struct Row {
    picture: &'static str,
    scene: &'static str,
    window: [f32; 2],
    start: Start,
}

const ROWS: [Row; 15] = [
    Row {
        picture: "ask-idle",
        scene: "idle",
        window: DEFAULT_WINDOW,
        start: Start::AsItOpens,
    },
    Row {
        picture: "ask-answer",
        scene: "black-scholes",
        window: DEFAULT_WINDOW,
        start: Start::AsItOpens,
    },
    Row {
        picture: "ask-answer-min",
        scene: "black-scholes",
        window: MIN_WINDOW,
        start: Start::AsItOpens,
    },
    Row {
        picture: "ask-source",
        scene: "black-scholes",
        window: DEFAULT_WINDOW,
        start: Start::AfterAClickOnTheFigureCitation,
    },
    Row {
        picture: "ask-no-sources",
        scene: "no-sources",
        window: DEFAULT_WINDOW,
        start: Start::AsItOpens,
    },
    Row {
        picture: "ask-answer-failed",
        scene: "answer-failed",
        window: DEFAULT_WINDOW,
        start: Start::AsItOpens,
    },
    Row {
        picture: "ask-stores-down",
        scene: "stores-down",
        window: DEFAULT_WINDOW,
        start: Start::AsItOpens,
    },
    Row {
        picture: "ask-first-run",
        scene: "first-run",
        window: DEFAULT_WINDOW,
        start: Start::AsItOpens,
    },
    Row {
        picture: "library",
        scene: "black-scholes",
        window: DEFAULT_WINDOW,
        start: Start::AfterAClickOnTheLibraryTab,
    },
    Row {
        picture: "ingest-ready",
        scene: "ingest-ready",
        window: DEFAULT_WINDOW,
        start: Start::AsItOpens,
    },
    Row {
        picture: "ingest-failed",
        scene: "ingest-failed",
        window: DEFAULT_WINDOW,
        start: Start::AfterAClickOnStartIngest,
    },
    Row {
        picture: "ingest-new-book",
        scene: "idle",
        window: DEFAULT_WINDOW,
        start: Start::AfterANewMediaIsTyped,
    },
    Row {
        picture: "ingest-running",
        scene: "ingest-running",
        window: DEFAULT_WINDOW,
        start: Start::AfterAStartThatNeverEnds,
    },
    Row {
        picture: "library-min",
        scene: "black-scholes",
        window: MIN_WINDOW,
        start: Start::AfterAClickOnTheLibraryTab,
    },
    Row {
        picture: "library-edit-media",
        scene: "black-scholes",
        window: DEFAULT_WINDOW,
        start: Start::AfterAClickOnAMediaPencil,
    },
];

/// Turns the wheel over the Answer pane until the chip is wholly inside it, then clicks it. A chip
/// that is cut by the edge of the pane is not what a person clicks.
fn click_the_figure_citation(harness: &mut Window, window: [f32; 2]) {
    let answer = layout::shell(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::Vec2::from(window),
    ))
    .answer;
    for _ in 0..40 {
        let chip = harness
            .query_all_by_role_and_label(Role::Button, FIGURE_CITATION)
            .next()
            .map(|chip| chip.rect());
        if chip.is_some_and(|rect| answer.contains_rect(rect)) {
            break;
        }
        harness.hover_at(answer.center());
        harness.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -240.0),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run_ok();
    }
    harness
        .get_by_role_and_label(Role::Button, FIGURE_CITATION)
        .click();
    testkit::settle(harness);
}

fn click(harness: &mut Window, role: Role, name: &str) {
    harness.get_by_role_and_label(role, name).click();
    testkit::settle(harness);
}

/// A box takes the text only when it has the keyboard, so it is clicked first.
fn type_into(harness: &mut Window, name: &str, text: &str) {
    harness.get_by_role_and_label(Role::TextInput, name).click();
    harness
        .get_by_role_and_label(Role::TextInput, name)
        .type_text(text);
    harness.run_ok();
}

/// The ingest never ends, so the app is never idle again, and `settle` would wait for ever.
fn start_an_ingest_that_never_ends(harness: &mut Window) {
    harness
        .get_by_role_and_label(Role::Button, START_INGEST)
        .click();
    let started = Instant::now();
    while harness
        .query_all_by_role_and_label(Role::ProgressIndicator, BAR_AT_PAGE_3)
        .next()
        .is_none()
    {
        assert!(started.elapsed() < LIMIT, "the bar never came to page 3");
        harness.run_ok();
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Escape at the end leaves no box with the keyboard: a box that has it draws a caret that
/// blinks, and the picture must be the same on every run.
fn type_a_new_media(harness: &mut Window) {
    click(harness, Role::Tab, "Ingest");
    click(harness, Role::ComboBox, "Media");
    click(harness, Role::Button, "Add new media…");
    type_into(harness, "Media title", "Natenberg on Options");
    type_into(harness, "Authors", "Sheldon Natenberg");
    type_into(harness, "Tags", "Volatility, options");
    harness.key_press(egui::Key::Escape);
    testkit::settle(harness);
}

impl Row {
    /// Draws the scene at rest and compares it with the kept picture. The error says how.
    fn compare(&self, options: &SnapshotOptions) -> Result<(), String> {
        let mut harness = testkit::app_at(self.scene, self.window, PIXELS_PER_POINT);
        testkit::settle(&mut harness);
        match self.start {
            Start::AsItOpens => {}
            Start::AfterAClickOnTheFigureCitation => {
                click_the_figure_citation(&mut harness, self.window);
            }
            Start::AfterAClickOnStartIngest => {
                harness
                    .get_by_role_and_label(Role::Button, START_INGEST)
                    .click();
                testkit::settle(&mut harness);
            }
            Start::AfterAStartThatNeverEnds => start_an_ingest_that_never_ends(&mut harness),
            Start::AfterANewMediaIsTyped => type_a_new_media(&mut harness),
            Start::AfterAClickOnTheLibraryTab => click(&mut harness, Role::Tab, "Library"),
            Start::AfterAClickOnAMediaPencil => {
                click(&mut harness, Role::Tab, "Library");
                click(&mut harness, Role::Button, FIRST_MEDIA_PENCIL);
            }
        }
        // The pointer is painted in the picture until a frame has run without it.
        harness.remove_cursor();
        harness.run_ok();
        harness
            .try_snapshot_options(self.picture, options)
            .map_err(|error| format!("{}: {error}", self.picture))
    }
}

#[test]
fn every_resting_scene_looks_as_it_was_approved() {
    let options = SnapshotOptions::new()
        .threshold(TOLERANCE)
        .output_path(FOLDER);
    let differing: Vec<String> = ROWS
        .iter()
        .filter_map(|row| row.compare(&options).err())
        .collect();
    assert!(
        differing.is_empty(),
        "{} of {} pictures differ from the approved ones:\n{}",
        differing.len(),
        ROWS.len(),
        differing.join("\n")
    );
}
