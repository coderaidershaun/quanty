//! The chrome that every panel starts with: the framed card, its header row and a divider.

use eframe::egui;

use crate::theme::{TextRole, color, radius, space};

/// The frame of every panel.
pub fn panel_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(color::PANEL)
        .stroke(egui::Stroke::new(1.0, color::HAIRLINE))
        .corner_radius(radius::XL)
        .inner_margin(space::LG)
}

/// A title at the left and whatever `add_actions` draws at the right.
pub fn section_header<R>(
    ui: &mut egui::Ui,
    title: &str,
    add_actions: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.horizontal(|ui| {
        ui.label(TextRole::Heading.rich(title));
        ui.with_layout(
            egui::Layout::right_to_left(egui::Align::Center),
            add_actions,
        )
        .inner
    })
    .inner
}

/// A line across the whole width.
pub fn separator(ui: &mut egui::Ui) {
    ui.separator();
}
