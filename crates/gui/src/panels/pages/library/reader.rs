//! The reader of the Library: the Source panel beside the open document, or in its place where the
//! room is too narrow, and the row that goes back from it on a phone.

use eframe::egui;

use super::{Local, back_row};
use crate::panels::{PanelCx, source};
use crate::theme::space;

/// Draws the Source panel in `rect`. The panel brings its own frame.
pub(super) fn show(ui: &mut egui::Ui, rect: egui::Rect, local: &mut Local, cx: &mut PanelCx<'_>) {
    let builder = egui::UiBuilder::new()
        .id_salt("library_reader")
        .max_rect(rect)
        .layout(egui::Layout::top_down(egui::Align::Min));
    ui.scope_builder(builder, |ui| {
        source::show_reader(ui, &mut local.reader, cx);
    });
}

/// The reader fills `room`, with the row that goes back to the document named `label` at its top.
pub(super) fn show_alone(
    ui: &mut egui::Ui,
    room: egui::Rect,
    label: &str,
    local: &mut Local,
    cx: &mut PanelCx<'_>,
) {
    let builder = egui::UiBuilder::new()
        .id_salt("library_reader")
        .max_rect(room)
        .layout(egui::Layout::top_down(egui::Align::Min));
    ui.scope_builder(builder, |ui| {
        back_row(ui, label, cx.intents);
        ui.add_space(space::SM);
        source::show_reader(ui, &mut local.reader, cx);
    });
}
