//! A stand-in for a part that is not built yet: the health dot, the bell and the help button,
//! and the sheets they open.

use eframe::egui;

use super::{PanelCx, placeholder};

#[derive(Debug, Default)]
pub struct Local {}

pub fn show(ui: &mut egui::Ui, _local: &mut Local, _cx: &mut PanelCx<'_>) {
    placeholder(ui, "Notices");
}
