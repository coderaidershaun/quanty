//! What the app lends to a panel while it draws, the one state each panel keeps for itself, and
//! the sheets that a page draws in.

use eframe::egui;

use super::{
    answer, ask_bar, concept_graph, datasets, follow_up, ingest, library, notices, retrieval_path,
    source,
};
use crate::contract::{Intent, Panel};
use crate::media::Media;
use crate::state::Shared;
use crate::theme::{Icon, TextRole, color, radius, stroke};
use crate::widgets::{Button, Surface, panel_frame};

/// The app applies the pushed `intents` after the frame.
pub struct PanelCx<'a> {
    pub shared: &'a Shared,
    pub media: &'a mut Media,
    pub intents: &'a mut Vec<Intent>,
}

impl PanelCx<'_> {
    /// The button that gives `panel` the whole tab, or puts every panel back while it has it.
    pub(super) fn show_maximise_toggle(&mut self, ui: &mut egui::Ui, panel: Panel) {
        let (icon, name, intent) = if self.shared.maximised == Some(panel) {
            (Icon::RESTORE, "Restore", Intent::RestorePanels)
        } else {
            (Icon::MAXIMISE, "Maximise", Intent::Maximise(panel))
        };
        if ui.add(Button::icon_only(icon, name)).clicked() {
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
    pub notices: notices::Local,
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
            ui.set_width(rect.width() - margin.x);
            add(ui, rect.height() - margin.y);
        });
    });
}

pub fn placeholder(ui: &mut egui::Ui, name: &str) {
    let rect = ui.max_rect();
    let response = ui.allocate_rect(rect, egui::Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, name));
    let painter = ui.painter();
    painter.rect(
        rect,
        radius::XL,
        color::SHEET,
        egui::Stroke::new(stroke::BORDER, color::HAIRLINE),
        egui::StrokeKind::Inside,
    );
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        name,
        TextRole::Heading.font(),
        color::TEXT_SECONDARY,
    );
}
