//! A row of controls that is cut at the edge of the room it is given. A control that is too
//! wide then cannot make the panel wider or push the rows below it out of place.

use eframe::egui;

pub(super) fn show<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let rest = ui.available_rect_before_wrap();
    let room = egui::Rect::from_min_size(
        rest.min,
        egui::vec2(rest.width(), ui.spacing().interact_size.y),
    );
    let mut used_height = 0.0;
    let inner = show_at(ui, room, |row| {
        let inner = add_contents(row);
        used_height = row.min_rect().height();
        inner
    });
    ui.allocate_space(egui::vec2(room.width(), used_height));
    inner
}

/// It takes no space from `ui`.
pub(super) fn show_at<R>(
    ui: &mut egui::Ui,
    room: egui::Rect,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let layout = egui::Layout::left_to_right(egui::Align::Center);
    let mut row = ui.new_child(egui::UiBuilder::new().max_rect(room).layout(layout));
    let sideways = egui::Rect::from_x_y_ranges(room.x_range(), ui.clip_rect().y_range());
    row.set_clip_rect(sideways.intersect(ui.clip_rect()));
    add_contents(&mut row)
}
