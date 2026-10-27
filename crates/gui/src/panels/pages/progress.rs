//! The row that says how far a job has come: its stage words, and what it has spent so far under
//! them when both do not fit.

use eframe::egui;

use crate::panels::labels::spent_text;
use crate::theme::{self, TextRole};
use contract::Usage;

/// The words of the stage, with what was spent so far at the right end of their line when both
/// fit on it, and under them when they do not. A job that has spent nothing shows only the words.
pub(super) fn stage_row(ui: &mut egui::Ui, words: &str, spent: Option<&Usage>) {
    let spent = spent.and_then(spent_so_far);
    let spent = spent.as_deref();
    let palette = theme::palette(ui.ctx());
    let words_color = TextRole::BodyStrong.color(palette);
    let words_width = TextRole::BodyStrong.galley(ui, words, words_color).size().x;
    let gap_and_spent_width = spent.map_or(0.0, |spent| {
        let width = TextRole::Small
            .galley(ui, spent, palette.text_muted)
            .size()
            .x;
        ui.spacing().item_spacing.x + width
    });
    if words_width + gap_and_spent_width > ui.available_width() {
        ui.label(TextRole::BodyStrong.rich(ui, words));
        if let Some(spent) = spent {
            ui.label(TextRole::Small.rich(ui, spent).color(palette.text_muted));
        }
        return;
    }
    ui.horizontal(|ui| {
        ui.label(TextRole::BodyStrong.rich(ui, words));
        if let Some(spent) = spent {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(TextRole::Small.rich(ui, spent).color(palette.text_muted));
            });
        }
    });
}

fn spent_so_far(spent: &Usage) -> Option<String> {
    (spent.tokens() > 0).then(|| format!("{} so far", spent_text(spent)))
}
