//! The button that stands for the filters with no room on the bar, with how many of them are set,
//! and the sheet it opens that holds them.

use eframe::egui;

use super::Local;
use super::filters::{Filter, Room};
use super::layout::{PICKER_WIDTH, Rects, slot};
use crate::theme::{TextRole, space, stroke};
use crate::widgets::{self, Button, ControlSize};

/// Twice as wide as a list on the bar, so a long title or name shows more of itself in the sheet.
const SHEET_WIDTH: f32 = 2.0 * PICKER_WIDTH;

impl Local {
    /// Draws the button for the filters that have no slot on the bar, and the sheet while it is
    /// open. Nothing is drawn when every filter has a slot.
    pub(super) fn show_more_filters(&mut self, ui: &mut egui::Ui, rects: &Rects) {
        let Some(button_rect) = rects.more else {
            self.is_sheet_open = false;
            return;
        };
        let word = if rects.filters.iter().any(Option::is_some) {
            "More filters"
        } else {
            "Filters"
        };
        let set_in_the_sheet = in_the_sheet(rects)
            .filter(|filter| filter.is_set(&self.draft.filters))
            .count();
        let label = match set_in_the_sheet {
            0 => word.to_owned(),
            count => format!("{word} ({count})"),
        };
        slot(ui, "more", button_rect, |ui| {
            let button = Button::secondary(&label)
                .size(rects.more_size)
                .min_width(button_rect.width())
                .accessible_name(word)
                .selected(set_in_the_sheet > 0);
            let response = ui.add(button);
            ui.ctx().accesskit_node_builder(response.id, |node| {
                node.set_value(set_in_the_sheet.to_string());
            });
            if response.clicked() {
                self.is_sheet_open = true;
            }
        });
        if self.is_sheet_open {
            self.show_sheet(ui.ctx(), rects, word);
        }
    }

    /// Done closes the sheet, and so do a click beside it and Escape.
    fn show_sheet(&mut self, ctx: &egui::Context, rects: &Rects, heading: &str) {
        // SMELL: the kit keeps the margin and the outline of its sheet to itself, so they are
        // repeated here. If the kit changes them, the sheet can be cut off at the sides of a phone.
        let side = 2.0 * (space::LG + space::XL + stroke::BORDER);
        let room = Room {
            width: SHEET_WIDTH.min(ctx.content_rect().width() - side),
            size: ControlSize::Medium,
        };
        let sheet = widgets::modal(ctx, egui::Id::new("ask bar filters"), |ui| {
            ui.set_width(room.width);
            ui.spacing_mut().item_spacing.y = space::SM;
            ui.label(TextRole::Heading.rich(heading));
            for filter in in_the_sheet(rects) {
                self.choices
                    .show_one(ui, filter, room, &mut self.draft.filters);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(Button::primary("Done").size(room.size)).clicked() {
                    ui.close();
                }
            });
        });
        if sheet.should_close() {
            self.is_sheet_open = false;
        }
    }
}

/// The filters with no slot on the bar, in the order of the filters.
fn in_the_sheet(rects: &Rects) -> impl Iterator<Item = Filter> {
    Filter::ALL
        .into_iter()
        .zip(rects.filters)
        .filter(|(_, slot)| slot.is_none())
        .map(|(filter, _)| filter)
}
