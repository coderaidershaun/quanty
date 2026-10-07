//! The page of the chapter that an answer stands on, with its pieces and concepts.

use eframe::egui;

use crate::panels::{PanelCx, placeholder};

#[derive(Debug, Default)]
pub struct Local {}

pub fn show(ui: &mut egui::Ui, _local: &mut Local, _cx: &mut PanelCx<'_>) {
    placeholder(ui, "Source in Context");
}
