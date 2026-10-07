//! The concepts of the answer and how they link, drawn as a graph.

use eframe::egui;

use crate::panels::{PanelCx, placeholder};

#[derive(Debug, Default)]
pub struct Local {}

pub fn show(ui: &mut egui::Ui, _local: &mut Local, _cx: &mut PanelCx<'_>) {
    placeholder(ui, "Concept Graph");
}
