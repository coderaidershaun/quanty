//! Checks the flows that cross two parts of the app, on the whole app: selection, follow-up,
//! source pages, failures, ingest, and that no control leads to a part that is not built.

mod dead_controls;
mod failures;
mod follow_up;
mod ingest;
mod recording;
mod selection;
mod source;

use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use egui_kittest::{Harness, Node};
use gui::app::App;
use gui::app::layout::{self, ShellRects};
use gui::state::{IngestJob, Shared};
use gui::testkit;

type Window = Harness<'static, App>;

fn shared(harness: &Window) -> &Shared {
    harness.state().shared()
}

fn panels(size: [f32; 2]) -> ShellRects {
    layout::shell(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::Vec2::from(size),
    ))
}

fn node<'a>(harness: &'a Window, role: Role, name: &'a str) -> Node<'a> {
    harness
        .query_all_by_role_and_label(role, name)
        .next()
        .unwrap_or_else(|| panic!("no {role:?} is named `{name}`"))
}

fn has(harness: &Window, role: Role, name: &str) -> bool {
    harness
        .query_all_by_role_and_label(role, name)
        .next()
        .is_some()
}

fn says(harness: &Window, words: &str) -> bool {
    harness.query_all_by_label_contains(words).next().is_some()
}

fn click(harness: &mut Window, role: Role, name: &str) {
    node(harness, role, name).click();
    testkit::settle(harness);
}

fn is_enabled(harness: &Window, role: Role, name: &str) -> bool {
    !node(harness, role, name).accesskit_node().is_disabled()
}

fn is_open_tab(harness: &Window, name: &str) -> bool {
    node(harness, Role::Tab, name)
        .accesskit_node()
        .is_selected()
        == Some(true)
}

#[derive(Clone, Copy)]
enum Wheel {
    Down,
    Up,
}

/// A row or a chip that is off screen has no node, so the wheel is turned until it is drawn.
fn scroll_to(harness: &mut Window, over: egui::Rect, wheel: Wheel, role: Role, name: &str) {
    let delta = match wheel {
        Wheel::Down => -240.0,
        Wheel::Up => 240.0,
    };
    for _ in 0..40 {
        if has(harness, role, name) {
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

fn node_in<'a>(
    harness: &'a Window,
    role: Role,
    name: &'a str,
    area: egui::Rect,
) -> Option<Node<'a>> {
    harness
        .query_all_by_role_and_label(role, name)
        .find(|node| area.contains(node.rect().center()))
}

fn failures(shared: &Shared) -> Vec<&gui::contract::Failure> {
    let ingest: Vec<&gui::contract::Failure> = match &shared.ingest {
        IngestJob::CheckFailed { failure, .. } => vec![failure],
        IngestJob::Checked { preflight, .. } => preflight.blockers.iter().collect(),
        IngestJob::Finished {
            result: Err(failure),
            ..
        } => vec![failure],
        _ => Vec::new(),
    };
    [
        shared.ask.search.failure(),
        shared.ask.graph.failure(),
        shared.ask.answer.failure(),
        shared.source.page.failure(),
        shared.source.concepts.failure(),
        shared.library.catalogue.failure(),
    ]
    .into_iter()
    .flatten()
    .chain(ingest)
    .collect()
}
