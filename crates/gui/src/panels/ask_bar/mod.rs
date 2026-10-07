//! The question box, with its filters and the Ask button.

mod filters;
mod layout;
mod note;
mod question;
#[cfg(test)]
mod tests;

use eframe::egui;

use crate::contract::AskDraft;
use crate::panels::PanelCx;
use crate::state::Shared;
use note::Note;

/// What the bar keeps between frames: the question being written, and what the filters offer.
#[derive(Debug, Default)]
pub struct Local {
    /// What the next ask sends.
    draft: AskDraft,
    /// The ask the draft was last loaded from.
    seen_generation: u64,
    /// The focus request already acted on.
    seen_focus_cue: u64,
    /// What the filters offer.
    choices: filters::Choices,
}

impl Local {
    /// Loads the draft from the ask the app holds whenever a new one starts, whoever started it,
    /// so a follow-up question shows in the box. Anything else leaves the draft alone.
    fn follow(&mut self, shared: &Shared) {
        let asked_anew = self.seen_generation != shared.ask.generation;
        if asked_anew {
            self.seen_generation = shared.ask.generation;
            self.draft = AskDraft {
                question: shared.ask.question.clone(),
                mode: shared.ask.mode,
                filters: shared.ask.filters.clone(),
            };
        }
        let library_changed = self.choices.refresh(&shared.library);
        if asked_anew || library_changed {
            self.choices.keep_known(&mut self.draft.filters);
        }
    }
}

/// Draws the bar into `ui`, which is its whole rectangle, and pushes an intent to ask or to stop.
pub fn show(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>) {
    local.follow(cx.shared);
    layout::card().show(ui, |ui| {
        let room = egui::vec2(ui.available_width(), layout::CONTENT_HEIGHT);
        let (content, _) = ui.allocate_exact_size(room, egui::Sense::hover());
        let rects = layout::rects(content);
        local.show_question(ui, &rects, cx);
        local.choices.show(ui, &rects, &mut local.draft.filters);
        if let Some(note) = Note::of(cx.shared, &local.draft) {
            note.show(ui, rects.note, cx.intents);
        }
    });
}
