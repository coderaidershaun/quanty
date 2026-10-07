//! The answer to the question, with its citations, and the results by kind.

use eframe::egui;

use crate::panels::{PanelCx, placeholder};

#[derive(Debug, Default)]
pub struct Local {}

pub fn show(ui: &mut egui::Ui, _local: &mut Local, _cx: &mut PanelCx<'_>) {
    placeholder(ui, "Answer");
}
