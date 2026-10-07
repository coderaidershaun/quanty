//! Questions to ask next, and the box to ask them in.

use eframe::egui;

use crate::panels::{PanelCx, placeholder};

#[derive(Debug, Default)]
pub struct Local {}

pub fn show(ui: &mut egui::Ui, _local: &mut Local, _cx: &mut PanelCx<'_>) {
    placeholder(ui, "Follow up");
}
