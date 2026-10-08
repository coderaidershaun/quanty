//! Keeps one part of the window inside the rectangle it is given. The window places its panels
//! with it, and the top bar places its logo and its tabs.

use eframe::egui;

pub(super) fn region(
    ui: &mut egui::Ui,
    name: &str,
    rect: egui::Rect,
    draw: impl FnOnce(&mut egui::Ui),
) {
    let builder = egui::UiBuilder::new()
        .id_salt(name)
        .max_rect(rect)
        .layout(egui::Layout::top_down(egui::Align::Min));
    ui.scope_builder(builder, |ui| {
        ui.set_clip_rect(rect);
        draw(ui);
    });
}
