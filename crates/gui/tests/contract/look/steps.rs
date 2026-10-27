//! How the app is brought to each resting state, and drawn once it is there.

use std::time::{Duration, Instant};

use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::Queryable as _;
use egui_kittest::{Harness, SnapshotOptions};
use gui::app::App;
use gui::testkit;

use super::rows::{BOOK_ROW, CHAPTER_ROW, Row, Start};
use crate::flows::panels;

type Window = Harness<'static, App>;

/// A kept picture is one pixel for each point, so that the folder stays small.
const PIXELS_PER_POINT: f32 = 1.0;

/// The chip of the figure's result. Its click is what a person does to open the source.
const FIGURE_CITATION: &str = "Citation 8";
const START_INGEST: &str = "Start ingest";
const SHARE: &str = "Share";
const EXPORT_AS_PDF: &str = "Export as PDF…";
const MAXIMISE: &str = "Maximise";
/// The pencil of the first media of the scene, whose form is on screen with no scroll.
const FIRST_MEDIA_PENCIL: &str = "Edit Hawkes Processes in Finance";
/// The Delete button of the same media, whose sheet is on screen with no scroll.
const FIRST_MEDIA_DELETE: &str = "Delete media Hawkes Processes in Finance";
/// The ingest of its scene stops for ever at page 3, so its bar rests there.
const BAR_AT_PAGE_3: &str = "Converting the pages — 3 of 12 done, about 36%";
/// The build of its scene stops for ever at the synthetic texts, so its bar rests there.
const BUILD_AT_ITS_STALL: &str = "Writing the synthetic texts — 6 of 12, about 50%";
const LIMIT: Duration = Duration::from_secs(5);

/// Turns the wheel over the Answer pane until the chip is wholly inside it, then clicks it. A chip
/// that is cut by the edge of the pane is not what a person clicks.
fn click_the_figure_citation(harness: &mut Window, window: [f32; 2]) {
    let answer = panels(window).answer;
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

/// The Source panel has a button of the same name, so this one is found by the panel it is in.
fn maximise_the_concept_graph(harness: &mut Window, window: [f32; 2]) {
    let graph = panels(window).concept_graph;
    harness
        .query_all_by_role_and_label(Role::Button, MAXIMISE)
        .find(|button| graph.contains(button.rect().center()))
        .expect("the Concept Graph has a Maximise button")
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

/// The build never ends, so the app is never idle, and `settle` would wait for ever. The footer
/// must have its words too, or one run could draw it checking and the next with what it found.
fn wait_for_the_stall_of_the_build(harness: &mut Window) {
    let started = Instant::now();
    loop {
        harness.run_ok();
        let shared = harness.state().shared();
        let is_loaded = shared.library.catalogue.ready().is_some() && !shared.balances.is_reading();
        let is_at_the_stall = harness
            .query_all_by_role_and_label(Role::ProgressIndicator, BUILD_AT_ITS_STALL)
            .next()
            .is_some();
        if is_loaded && is_at_the_stall {
            return;
        }
        assert!(started.elapsed() < LIMIT, "the bar never came to its stall");
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
    pub(super) fn compare(&self, options: &SnapshotOptions) -> Result<(), String> {
        let home = tempfile::tempdir().unwrap();
        if matches!(self.start, Start::AfterAnExportThatFails) {
            std::fs::write(home.path().join("data"), "not a folder").unwrap();
        }
        let mut harness =
            testkit::app_at_with_home(self.scene, self.window, PIXELS_PER_POINT, home);
        harness.state_mut().set_scheme(self.scheme);
        match self.start {
            Start::AtTheStallOfItsBuild => wait_for_the_stall_of_the_build(&mut harness),
            _ => testkit::settle(&mut harness),
        }
        match self.start {
            Start::AsItOpens | Start::AtTheStallOfItsBuild => {}
            Start::AfterAClickOnTheFigureCitation => {
                click_the_figure_citation(&mut harness, self.window);
            }
            Start::AfterAClickOnShare => click(&mut harness, Role::Button, SHARE),
            Start::AfterAnExportThatFails => {
                click(&mut harness, Role::Button, SHARE);
                click(&mut harness, Role::Button, EXPORT_AS_PDF);
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
            Start::AfterOpeningAViewOfAChapter {
                media,
                chapter,
                view,
            } => {
                click(&mut harness, Role::Tab, "Library");
                click(&mut harness, Role::Button, media);
                click(&mut harness, Role::Button, chapter);
                click(&mut harness, Role::Tab, view);
            }
            Start::AfterAClickOnAMediaPencil => {
                click(&mut harness, Role::Tab, "Library");
                click(&mut harness, Role::Button, FIRST_MEDIA_PENCIL);
            }
            Start::AfterAClickOnAMediaDelete => {
                click(&mut harness, Role::Tab, "Library");
                click(&mut harness, Role::Button, FIRST_MEDIA_DELETE);
            }
            Start::AfterAClickOnAChapterRow => {
                click(&mut harness, Role::Tab, "Library");
                click(&mut harness, Role::Button, BOOK_ROW);
                click(&mut harness, Role::Button, CHAPTER_ROW);
            }
            Start::AfterAClickOnMaximiseInTheConceptGraph => {
                maximise_the_concept_graph(&mut harness, self.window);
            }
            Start::AfterAClickOn { role, name } => click(&mut harness, role, name),
        }
        // The pointer is painted in the picture until a frame has run without it.
        harness.remove_cursor();
        harness.run_ok();
        testkit::save_png(&mut harness, self.picture);
        harness
            .try_snapshot_options(self.picture, options)
            .map_err(|error| format!("{}: {error}", self.picture))
    }
}
