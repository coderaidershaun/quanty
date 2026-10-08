//! The Ingest page on the whole app and the fake backend: a PDF is checked as soon as it and
//! its media are known, a media is saved and edited before its first PDF, and an ingest runs to
//! its end.

mod check;
mod media;
mod run;

use eframe::egui;
use eframe::egui::accesskit::Role;
use gui::contract::{Command, IngestRequest};
use gui::testkit;

use super::recording::{self, Seen};
use super::{Window, click, press};

fn is_preflight(command: &Command) -> bool {
    matches!(command, Command::Preflight { .. })
}

fn is_ingest(command: &Command) -> bool {
    matches!(command, Command::Ingest { .. })
}

fn open(scene: &str, size: [f32; 2]) -> (Window, Seen) {
    let (mut harness, seen) = recording::open(scene, size);
    testkit::settle(&mut harness);
    (harness, seen)
}

fn place_of(seen: &Seen, is: fn(&Command) -> bool) -> Option<usize> {
    seen.all().iter().position(is)
}

fn open_the_form_of_a_new_media(harness: &mut Window) {
    click(harness, Role::ComboBox, "Media");
    click(harness, Role::Button, "Add new media…");
}

/// A row of the list reads "Title · Category".
fn choose_in_the_media_list(harness: &mut Window, row: &str) {
    click(harness, Role::ComboBox, "Media");
    click(harness, Role::Button, row);
}

fn close_the_open_list(harness: &mut Window) {
    press(harness, egui::Modifiers::NONE, egui::Key::Escape);
}

fn last_checked(seen: &Seen) -> IngestRequest {
    let checked = seen
        .all()
        .into_iter()
        .rev()
        .find_map(|command| match command {
            Command::Preflight { ingest, .. } => Some(ingest),
            _ => None,
        });
    checked.expect("the chapter was not checked")
}
