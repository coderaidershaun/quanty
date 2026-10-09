//! What the app lends to a panel while it draws, and the one state each panel keeps for itself.

use eframe::egui;

use super::{
    answer, ask_bar, concept_graph, follow_up, ingest, library, notices, retrieval_path, source,
};
use crate::contract::{Intent, Panel};
use crate::media::Media;
use crate::state::Shared;
use crate::theme::{Icon, TextRole, color, radius};
use crate::widgets::Button;

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
    pub notices: notices::Local,
}

pub fn placeholder(ui: &mut egui::Ui, name: &str) {
    let rect = ui.max_rect();
    let response = ui.allocate_rect(rect, egui::Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, name));
    let painter = ui.painter();
    painter.rect(
        rect,
        radius::XL,
        color::PANEL,
        egui::Stroke::new(1.0, color::HAIRLINE),
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
