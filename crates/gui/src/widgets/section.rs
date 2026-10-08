//! The chrome that every panel starts with: the framed card, its header row and a divider.

use eframe::egui;

use crate::theme::{TextRole, color, hairline, radius, size, space, stroke};

pub fn panel_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(color::PANEL)
        .stroke(egui::Stroke::new(stroke::BORDER, color::HAIRLINE))
        .corner_radius(radius::XL)
        .inner_margin(space::LG)
}

/// The actions are laid out from right to left, so the first one that is drawn is the one at
/// the far right.
pub fn section_header<R>(
    ui: &mut egui::Ui,
    title: &str,
    add_actions: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.horizontal(|ui| {
        ui.set_min_height(size::CONTROL_SM);
        ui.label(TextRole::Heading.rich(title));
        ui.with_layout(
            egui::Layout::right_to_left(egui::Align::Center),
            add_actions,
        )
        .inner
    })
    .inner
}

pub fn separator(ui: &mut egui::Ui) {
    let side = egui::vec2(ui.available_width(), stroke::BORDER);
    let (rect, _) = ui.allocate_exact_size(side, egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.hline(
            rect.x_range(),
            rect.center().y,
            hairline(painter, color::HAIRLINE),
        );
    }
}
