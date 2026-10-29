//! The strip between two panes that is dragged to make the pane on its left wider or narrower.

use std::ops::RangeInclusive;

use eframe::egui;

use crate::theme::{self, space, stroke};

/// The strip right of `pane` is a handle named `label`. While it is dragged, this returns the width
/// the pane takes to follow the pointer, kept within `range`. The edge shows under the pointer.
/// Each handle in a `ui` needs its own label, because the label also tells the handles apart.
pub(super) fn drag(
    ui: &mut egui::Ui,
    pane: egui::Rect,
    label: &str,
    range: RangeInclusive<f32>,
) -> Option<f32> {
    let handle = egui::Rect::from_min_max(
        pane.right_top(),
        egui::pos2(pane.right() + space::MD, pane.bottom()),
    );
    let response = ui
        .interact(handle, ui.id().with(label), egui::Sense::drag())
        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(egui::accesskit::Role::Splitter);
        node.set_label(label);
    });
    // The pointer is taken to hold the middle of the gap, so a press that does not move keeps the
    // width.
    let width = response
        .interact_pointer_pos()
        .filter(|_| response.dragged())
        .map(|pointer| {
            (pointer.x - pane.left() - space::MD / 2.0).clamp(*range.start(), *range.end())
        });
    if response.hovered() || response.dragged() {
        let palette = theme::palette(ui.ctx());
        let color = if response.dragged() {
            palette.focus
        } else {
            palette.border
        };
        let x = handle.center().x;
        ui.painter().line_segment(
            [egui::pos2(x, handle.top()), egui::pos2(x, handle.bottom())],
            egui::Stroke::new(stroke::BORDER, color),
        );
    }
    width
}
