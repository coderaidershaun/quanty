//! The row that says how far a job has come: its stage words, and what it has spent so far under
//! them when both do not fit.

use eframe::egui;

use crate::panels::labels::spent_text;
use crate::theme::{TextRole, color};
use contract::Usage;

/// The words of the stage, with what was spent so far at the right end of their line when both
/// fit on it, and under them when they do not. A job that has spent nothing shows only the words.
pub(super) fn stage_row(ui: &mut egui::Ui, words: &str, spent: Option<&Usage>) {
    let spent = spent.and_then(spent_so_far);
    let spent = spent.as_deref();
    let words_width = TextRole::BodyStrong
        .galley(ui, words, TextRole::BodyStrong.color())
        .size()
        .x;
    let gap_and_spent_width = spent.map_or(0.0, |spent| {
        let width = TextRole::Small
            .galley(ui, spent, color::TEXT_MUTED)
            .size()
            .x;
        ui.spacing().item_spacing.x + width
    });
    if words_width + gap_and_spent_width > ui.available_width() {
        ui.label(TextRole::BodyStrong.rich(words));
        if let Some(spent) = spent {
            ui.label(TextRole::Small.rich(spent).color(color::TEXT_MUTED));
        }
        return;
    }
    ui.horizontal(|ui| {
        ui.label(TextRole::BodyStrong.rich(words));
        if let Some(spent) = spent {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(TextRole::Small.rich(spent).color(color::TEXT_MUTED));
            });
        }
    });
}

fn spent_so_far(spent: &Usage) -> Option<String> {
    (spent.tokens() > 0).then(|| format!("{} so far", spent_text(spent)))
}
