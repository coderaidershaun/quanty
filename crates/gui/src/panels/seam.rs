//! What the app lends to a panel while it draws, the one state each panel keeps for itself, and
//! the sheets that a page draws in.

use eframe::egui;

use super::{
    answer, ask_bar, concept_graph, datasets, follow_up, ingest, library, retrieval_path, source,
};
use crate::media::Media;
use crate::state::Shared;
use crate::theme::{Icon, size};
use crate::widgets::{Button, ControlSize, Surface, panel_frame};
use contract::{Intent, Panel};

/// Whether a panel is narrow, judged by its own width and not by the window's. A narrow panel
/// stacks its controls and makes the main ones tall enough for a finger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PanelWidth {
    Narrow,
    Roomy,
}

impl PanelWidth {
    /// The width left in `ui` against the width under which this panel is narrow.
    pub(super) fn of(ui: &egui::Ui, narrow_below: f32) -> PanelWidth {
        if ui.available_width() < narrow_below {
            PanelWidth::Narrow
        } else {
            PanelWidth::Roomy
        }
    }

    pub(super) fn control_size(self) -> ControlSize {
        match self {
            PanelWidth::Narrow => ControlSize::Medium,
            PanelWidth::Roomy => ControlSize::Small,
        }
    }

    /// The height of a chip that a finger presses, for `Chip::min_height`.
    pub(super) fn chip_height(self) -> f32 {
        match self {
            PanelWidth::Narrow => size::CONTROL_MD,
            PanelWidth::Roomy => size::CHIP,
        }
    }
}

/// The app applies the pushed `intents` after the frame.
pub struct PanelCx<'a> {
    pub shared: &'a Shared,
    pub media: &'a mut Media,
    pub intents: &'a mut Vec<Intent>,
}

impl PanelCx<'_> {
    /// The button that gives `panel` the whole tab, or puts every panel back while it has it.
    pub(super) fn show_maximise_toggle(
        &mut self,
        ui: &mut egui::Ui,
        panel: Panel,
        size: ControlSize,
    ) {
        let (icon, name, intent) = if self.shared.maximised == Some(panel) {
            (Icon::RESTORE, "Restore", Intent::RestorePanels)
        } else {
            (Icon::MAXIMISE, "Maximise", Intent::Maximise(panel))
        };
        if ui.add(Button::icon_only(icon, name).size(size)).clicked() {
            self.intents.push(intent);
        }
    }
}

/// The own state of every panel, kept by the app between frames.
#[derive(Debug, Default)]
pub struct Locals {
    pub ask_bar: ask_bar::Local,
    pub answer: answer::Local,
    pub source: source::Local,
    pub concept_graph: concept_graph::Local,
    pub retrieval_path: retrieval_path::Local,
    pub follow_up: follow_up::Local,
    pub library: library::Local,
    pub ingest: ingest::Local,
    pub datasets: datasets::Local,
}

/// A framed column in the middle of the room. `add` draws in it and gets the height inside it.
pub(super) fn column(
    ui: &mut egui::Ui,
    id_salt: &str,
    max_width: f32,
    add: impl FnOnce(&mut egui::Ui, f32),
) {
    let page = ui.max_rect();
    let width = page.width().min(max_width);
    let column = egui::Rect::from_min_size(
        egui::pos2(page.center().x - width / 2.0, page.top()),
        egui::vec2(width, page.height()),
    );
    framed(ui, id_salt, column, add);
}

/// A sheet that fills `rect`. `add` draws in it and gets the height inside it.
pub(super) fn framed(
    ui: &mut egui::Ui,
    id_salt: &str,
    rect: egui::Rect,
    add: impl FnOnce(&mut egui::Ui, f32),
) {
    let frame = panel_frame(Surface::Sheet);
    let margin = frame.total_margin().sum();
    let builder = egui::UiBuilder::new()
        .id_salt(id_salt)
        .max_rect(rect)
        .layout(egui::Layout::top_down(egui::Align::Min));
    ui.scope_builder(builder, |ui| {
        frame.show(ui, |ui| {
            // A rect smaller than its own margin has no inside, and egui refuses a negative width.
            ui.set_width((rect.width() - margin.x).max(0.0));
            add(ui, (rect.height() - margin.y).max(0.0));
        });
    });
}
