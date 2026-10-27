//! Checks the flows that cross two parts of the app, on the whole app: the Ask tab, failures,
//! ingest, datasets, the library, model choices, the footer's balances, the app on small
//! screens, and that no control leads to a part that is not built.

mod ask;
mod balances;
mod datasets;
mod dead_controls;
mod failures;
pub(super) mod fit;
mod ingest;
mod library;
mod model_choice;
mod recording;
mod small_screens;

use contract::{Catalogue, Category, Command, DatasetRequest, DatasetStage, Filters, Tab};
use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use egui_kittest::{Harness, Node};
use gui::app::App;
use gui::app::layout::{self, NoteRow, ShellRects};
use gui::state::{DatasetJob, IngestJob, Shared};
use gui::testkit;
use recording::Seen;

pub(super) type Window = Harness<'static, App>;

const COMMAND: egui::Modifiers = egui::Modifiers::COMMAND;

pub(super) fn shared(harness: &Window) -> &Shared {
    harness.state().shared()
}

/// The rectangles of a window of that size whose Ask bar holds no note, as in every scene whose
/// library has loaded and holds a document.
pub(super) fn panels(size: [f32; 2]) -> ShellRects {
    layout::shell(fit::window_rect(size), NoteRow::Absent)
}

/// The rectangles the app drew last. A phone's Ask bar is as tall as the note it holds, so a test
/// that reads it while a note may show asks the app, not `panels`.
pub(super) fn drawn(harness: &Window) -> ShellRects {
    harness.state().shell_rects()
}

fn node<'a>(harness: &'a Window, role: Role, name: &'a str) -> Node<'a> {
    harness
        .query_all_by_role_and_label(role, name)
        .next()
        .unwrap_or_else(|| panic!("no {role:?} is named `{name}`"))
}

pub(super) fn has(harness: &Window, role: Role, name: &str) -> bool {
    harness
        .query_all_by_role_and_label(role, name)
        .next()
        .is_some()
}

fn says(harness: &Window, words: &str) -> bool {
    harness.query_all_by_label_contains(words).next().is_some()
}

/// Whether the nodes of that role and those names stand on the screen from the top down in the
/// order given.
fn are_stacked_in_order(harness: &Window, role: Role, names: &[&str]) -> bool {
    let tops: Vec<f32> = names
        .iter()
        .map(|name| node(harness, role, name).rect().top())
        .collect();
    tops.is_sorted_by(|above, below| above < below)
}

fn click(harness: &mut Window, role: Role, name: &str) {
    node(harness, role, name).click();
    testkit::settle(harness);
}

fn is_enabled(harness: &Window, role: Role, name: &str) -> bool {
    !node(harness, role, name).accesskit_node().is_disabled()
}

fn field(harness: &Window, name: &str) -> Option<String> {
    node(harness, Role::TextInput, name).value()
}

/// A box takes the text only when it has the keyboard, so it is clicked first.
fn type_into(harness: &mut Window, name: &str, text: &str) {
    node(harness, Role::TextInput, name).click();
    node(harness, Role::TextInput, name).type_text(text);
    harness.run_ok();
}

/// A box takes the text only when it has the keyboard. All of its text is selected first, so what
/// is typed replaces what the box held.
fn retype(harness: &mut Window, name: &str, text: &str) {
    click(harness, Role::TextInput, name);
    harness.key_press_modifiers(COMMAND, egui::Key::A);
    harness.step();
    node(harness, Role::TextInput, name).type_text(text);
    harness.run_ok();
}

fn copied(harness: &Window) -> Vec<String> {
    harness
        .output()
        .platform_output
        .commands
        .iter()
        .filter_map(|command| match command {
            egui::OutputCommand::CopyText(text) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// Runs the frames of the events already queued, and returns what they copied. The copy is in
/// the output of the last frame only, so no frame may run after the one that asks for it.
fn copied_by(harness: &mut Window, act: impl FnOnce(&mut Window)) -> Vec<String> {
    act(harness);
    harness.step();
    let texts = copied(harness);
    testkit::settle(harness);
    texts
}

fn is_open_tab(harness: &Window, name: &str) -> bool {
    node(harness, Role::Tab, name)
        .accesskit_node()
        .is_selected()
        == Some(true)
}

/// Every button of a card that deletes: `Delete document …` and `Delete media …`.
fn delete_buttons(harness: &Window) -> Vec<Node<'_>> {
    harness
        .query_all_by_role(Role::Button)
        .filter(|button| {
            let name = button.accesskit_node().label();
            name.is_some_and(|name| name.starts_with("Delete "))
        })
        .collect()
}

/// The text of a button that is off shows once the pointer has rested on it, and not for the
/// button that was clicked last, so a test hovers another one. The app is never at rest when this
/// is used, so `run_ok` runs the four frames that the rest takes.
fn hover_on(harness: &mut Window, role: Role, name: &str) {
    node(harness, role, name).hover();
    harness.run_ok();
}

#[derive(Clone, Copy)]
pub(super) enum Wheel {
    Down,
    Up,
}

/// A row or a chip that is off screen has no node, so the wheel is turned until a node of that
/// name is wholly inside `over`.
pub(super) fn scroll_to(
    harness: &mut Window,
    over: egui::Rect,
    wheel: Wheel,
    role: Role,
    name: &str,
) {
    let delta = match wheel {
        Wheel::Down => -240.0,
        Wheel::Up => 240.0,
    };
    for _ in 0..40 {
        if harness
            .query_all_by_role_and_label(role, name)
            .any(|node| over.contains_rect(node.rect()))
        {
            return;
        }
        harness.hover_at(over.center());
        harness.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, delta),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run_ok();
    }
    panic!("`{name}` never came into view");
}

fn press(harness: &mut Window, modifiers: egui::Modifiers, key: egui::Key) {
    harness.key_press_modifiers(modifiers, key);
    harness.step();
    testkit::settle(harness);
}

pub(super) fn node_in<'a>(
    harness: &'a Window,
    role: Role,
    name: &'a str,
    area: egui::Rect,
) -> Option<Node<'a>> {
    harness
        .query_all_by_role_and_label(role, name)
        .find(|node| area.contains(node.rect().center()))
}

fn failures(shared: &Shared) -> Vec<&contract::Failure> {
    let ingest: Vec<&contract::Failure> = match &shared.ingest {
        IngestJob::CheckFailed { failure, .. } => vec![failure],
        IngestJob::Checked { preflight, .. } => preflight.blockers.iter().collect(),
        IngestJob::Finished {
            result: Err(failure),
            ..
        } => vec![failure],
        _ => Vec::new(),
    };
    let dataset_build = match &shared.datasets {
        DatasetJob::Finished {
            result: Err(failure),
            ..
        } => Some(failure),
        _ => None,
    };
    [
        shared.ask.search.failure(),
        shared.ask.graph.failure(),
        shared.ask.answer.failure(),
        shared.source.page.failure(),
        shared.source.concepts.failure(),
        shared.library.catalogue.failure(),
        shared.contents.items.failure(),
        shared.contents.concepts.failure(),
        shared.insights.saved.failure(),
    ]
    .into_iter()
    .flatten()
    .chain(ingest)
    .chain(dataset_build)
    .collect()
}

/// Opens a tab as a person does, by pressing it in the top bar.
fn open_tab(harness: &mut Window, tab: Tab) {
    let name = match tab {
        Tab::Ask => "Ask",
        Tab::Library => "Library",
        Tab::Ingest => "Ingest",
        Tab::Datasets => "Datasets",
    };
    click_in(harness, Role::Tab, name, drawn(harness).top_bar);
}

/// The Ask bar and the Source panel each have a list named "Media", so a control is found by the
/// panel it is in.
pub(super) fn click_in(harness: &mut Window, role: Role, name: &str, area: egui::Rect) {
    node_in(harness, role, name, area)
        .unwrap_or_else(|| panic!("no {role:?} named `{name}` is in {area:?}"))
        .click();
    testkit::settle(harness);
}

fn question_box(harness: &Window) -> Option<String> {
    node(harness, Role::TextInput, "Question").value()
}

/// The paper of the library, by its title, its first author and its first tag.
struct Paper {
    title: String,
    author: String,
    tag: String,
}

impl Paper {
    fn of(catalogue: &Catalogue) -> Paper {
        let paper = catalogue
            .media
            .iter()
            .find(|media| media.category == Category::Paper)
            .expect("the library has a paper");
        Paper {
            title: paper.title.clone().expect("the paper has a title"),
            author: paper
                .authors
                .first()
                .cloned()
                .expect("the paper has authors"),
            tag: paper.tags.first().cloned().expect("the paper has a tag"),
        }
    }

    fn filters(&self) -> Filters {
        Filters {
            document: None,
            media: Some(self.title.clone()),
            author: Some(self.author.clone()),
            tags: vec![self.tag.clone()],
            category: Some(Category::Paper),
        }
    }
}

fn filters_of_the_last_ask(seen: &Seen) -> Option<Filters> {
    seen.all()
        .into_iter()
        .rev()
        .find_map(|command| match command {
            Command::Ask { ask, .. } => Some(ask.filters),
            _ => None,
        })
}

/// The stage at which the scenes with a build that never ends stand still.
const STALL: DatasetStage = DatasetStage::WritingSynthetic { done: 6, total: 12 };

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

fn is_preflight(command: &Command) -> bool {
    matches!(command, Command::Preflight { .. })
}
