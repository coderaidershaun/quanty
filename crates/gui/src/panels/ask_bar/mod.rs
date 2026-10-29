//! The question box, with its filters and the Ask button.

mod filters;
mod layout;
mod note;
mod question;
mod sheet;

use eframe::egui;

use crate::panels::PanelCx;
use crate::state::Shared;
use contract::AskDraft;
pub(crate) use layout::{TWO_ROWS, rows_height};
use note::Note;
pub use note::NoteRow;
pub(crate) use note::note_row;

#[derive(Debug, Default)]
pub struct Local {
    draft: AskDraft,
    seen_generation: u64,
    seen_focus_cue: u64,
    choices: filters::Choices,
    is_sheet_open: bool,
}

impl Local {
    /// Loads the draft from the ask the app holds whenever a new one starts, whoever started it,
    /// so a follow-up question shows in the box.
    fn follow(&mut self, shared: &Shared) {
        let asked_anew = self.seen_generation != shared.ask.generation;
        if asked_anew {
            self.seen_generation = shared.ask.generation;
            self.draft = AskDraft {
                question: shared.ask.question.clone(),
                mode: shared.ask.mode,
                depth: shared.ask.depth,
                filters: shared.ask.filters.clone(),
            };
        }
        let library_changed = self.choices.refresh(&shared.library);
        if asked_anew || library_changed {
            self.choices.keep_known(&mut self.draft.filters);
        }
    }
}

pub fn show(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>) {
    local.follow(cx.shared);
    // The bar has no frame of its own, because the top line of a frame on the desk would sit just
    // under the line of the top bar.
    let (bar, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
    let rects = layout::arrange(bar, note_row(cx.shared));
    local.show_question(ui, &rects, cx);
    local.choices.show(ui, &rects, &mut local.draft.filters);
    local.show_more_filters(ui, &rects);
    if let (Some(note), Some(slot)) = (Note::of(cx.shared, &local.draft), rects.note) {
        note.show(ui, slot, rects.note_size, cx.intents);
    }
}
