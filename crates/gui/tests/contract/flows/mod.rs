//! Checks the flows that cross two parts of the app, on the whole app: a choice made in one
//! panel shows in the others, a follow-up starts a new ask, a source page turns, a service that
//! is down is named, and no control leads to a part that is not built.

mod dead_controls;
mod failures;
mod follow_up;
mod recording;
mod selection;
mod source;

use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use egui_kittest::{Harness, Node};
use gui::app::App;
use gui::app::layout::{self, ShellRects};
use gui::state::Shared;
use gui::testkit;

type Window = Harness<'static, App>;

fn shared(harness: &Window) -> &Shared {
    harness.state().shared()
}

/// Where each panel sits at this window size.
fn panels(size: [f32; 2]) -> ShellRects {
    layout::shell(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::Vec2::from(size),
    ))
}

/// The first node with this role and name.
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

/// True when some node has `words` in its name.
fn says(harness: &Window, words: &str) -> bool {
    harness.query_all_by_label_contains(words).next().is_some()
}

/// Clicks the first node with this role and name, then waits for the app to be idle.
fn click(harness: &mut Window, role: Role, name: &str) {
    node(harness, role, name).click();
    testkit::settle(harness);
}

/// True when the first node with this role and name is not faded out.
fn is_enabled(harness: &Window, role: Role, name: &str) -> bool {
    !node(harness, role, name).accesskit_node().is_disabled()
}

/// True when the tab of this name is the open one.
fn is_open_tab(harness: &Window, name: &str) -> bool {
    node(harness, Role::Tab, name)
        .accesskit_node()
        .is_selected()
        == Some(true)
}

/// Which way the wheel turns.
#[derive(Clone, Copy)]
enum Wheel {
    Down,
    Up,
}

/// Turns the wheel over `over` until a node with this role and name is drawn: a row or a chip
/// that is off screen has no node.
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

/// Presses a key, with one frame for the key alone, then lets the app finish what it started.
fn press(harness: &mut Window, modifiers: egui::Modifiers, key: egui::Key) {
    harness.key_press_modifiers(modifiers, key);
    harness.step();
    testkit::settle(harness);
}

/// The first node with this role and name whose middle is inside `area`.
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

/// Every failure that the shared state holds: the three slots of the ask, the two of Source and
/// the catalogue.
fn failures(shared: &Shared) -> Vec<&gui::contract::Failure> {
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
    .collect()
}
