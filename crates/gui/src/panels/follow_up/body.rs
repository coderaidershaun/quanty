//! Draws what shows under the box: one message, then the questions to ask next.

use eframe::egui;

use super::guidance::{self, Guidance, Message};
use crate::theme::{TextRole, size, space};
use crate::widgets;

/// `height` is the room under the box.
pub(super) fn show<'a>(ui: &mut egui::Ui, guidance: &Guidance<'a>, height: f32) -> Option<&'a str> {
    let top = ui.cursor().top();
    if let Some(message) = &guidance.message {
        show_message(ui, message);
    }
    let height_under_message = height - (ui.cursor().top() - top);
    show_chips(ui, guidance.suggestions, height_under_message)
}

fn show_chips<'a>(ui: &mut egui::Ui, suggestions: &'a [String], height: f32) -> Option<&'a str> {
    if suggestions.is_empty() {
        return None;
    }
    let width = ui.available_width();
    let mut clicked = None;
    ui.scope(|ui| {
        let spacing = ui.spacing_mut();
        // A wrapped row is never lower than this. Left at a theme's taller value, four chips
        // would not fit the smallest window.
        spacing.interact_size.y = size::CHIP;
        spacing.item_spacing.y = gap(height, suggestions.len());
        ui.horizontal_wrapped(|ui| {
            for question in suggestions {
                let chip = widgets::Chip::plain(question).max_width(width);
                if ui.add(chip).clicked() {
                    clicked = Some(question.as_str());
                }
            }
        });
    });
    clicked
}

/// Up to four one-row chips must show in the smallest window, so the gap gives way first.
fn gap(height: f32, chips: usize) -> f32 {
    let between = chips.saturating_sub(1).max(1) as f32;
    ((height - chips as f32 * size::CHIP) / between)
        .floor()
        .clamp(space::XS, space::SM)
}

fn show_message(ui: &mut egui::Ui, message: &Message<'_>) {
    match message {
        Message::Waiting(text) => {
            ui.horizontal(|ui| {
                widgets::spinner(ui, guidance::WAITING_LABEL);
                ui.label(TextRole::Small.rich(*text));
            });
        }
        Message::Line { text, hint } => {
            ui.label(TextRole::Small.rich(*text));
            if let Some(hint) = hint {
                ui.label(TextRole::Small.rich(*hint));
            }
        }
        Message::Notice { title, body } => {
            widgets::Notice::info(title).body(body).show(ui);
        }
    }
}
