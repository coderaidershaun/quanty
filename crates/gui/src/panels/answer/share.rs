//! Share and the menu it opens: copy the answer as Markdown, or export it as a PDF, a Markdown
//! file or an HTML file.

use eframe::egui;

use super::pane::{Copied, Pane};
use super::phase::{Phase, Written};
use crate::panels::exporting;
use crate::theme::{Icon, size};
use crate::widgets::{Button, ControlSize};

const SHARE_WIDTH: f32 = 3.0 * size::CONTROL_MD;

pub(super) fn show(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>) {
    let button = Button::secondary("Share")
        .icon(Icon::SHARE)
        .size(ControlSize::Small)
        .min_width(SHARE_WIDTH);
    let has_answer = matches!(
        pane.phase,
        Phase::Found {
            written: Written::Ready(_)
        }
    );
    let response = ui
        .add_enabled(has_answer, button)
        .on_disabled_hover_text("There is no answer to share yet.");
    let ask = pane.ask;
    egui::Popup::menu(&response)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            copy_item(ui, pane);
            if exporting::items(ui, pane.cx, |format| ask.export_request(format)) {
                ui.close();
            }
        });
}

/// The menu stays open after a copy, and the item reads "Copied" until the pointer leaves it.
fn copy_item(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>) {
    let is_copied = pane.view.copied == Some(Copied::Answer);
    let (label, icon) = if is_copied {
        ("Copied", Icon::CHECK)
    } else {
        ("Copy as Markdown", Icon::COPY)
    };
    let item = Button::ghost(label)
        .icon(icon)
        .size(ControlSize::Small)
        .min_width(exporting::ITEM_WIDTH)
        .start_aligned();
    let response = ui.add(item);
    if response.clicked() {
        pane.share();
    } else if is_copied && !ui.rect_contains_pointer(response.rect) {
        pane.view.copied = None;
    }
}
