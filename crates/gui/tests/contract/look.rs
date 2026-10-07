//! The look of the app at rest is kept as pictures on disk, and a change that moves a pixel by
//! more than the tolerance fails here until a person looks at the new picture and approves it.

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

enum Start {
    AsItOpens,
    AfterAClickOnTheFigureCitation,
    AfterAClickOnStartIngest,
}

struct Row {
    picture: &'static str,
    scene: &'static str,
    window: [f32; 2],
    start: Start,
}

const ROWS: [Row; 10] = [
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
