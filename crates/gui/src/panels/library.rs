//! A stand-in for the Library page, which is not built yet: the stored documents, grouped by book,
//! with their labels.

use eframe::egui;

use super::{PanelCx, placeholder};

#[derive(Debug, Default)]
pub struct Local {}

pub fn show(ui: &mut egui::Ui, _local: &mut Local, _cx: &mut PanelCx<'_>) {
    placeholder(ui, "Library");
}
