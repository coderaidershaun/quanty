//! The form that corrects the own tags of one document, and how its save went.

use eframe::egui;

use super::{CANCEL, SAVE, SaveStep, reveal};
use crate::contract::{DocId, Document, DocumentTagsEdit, Intent};
use crate::panels::labels::{TAGS, caption, list_of, text_of};
use crate::state::{Library, Shared};
use crate::theme::space;
use crate::widgets::{Button, Notice, TextInput};

const TAGS_HINT: &str = "With commas between them";

#[derive(Debug)]
pub(super) struct Draft {
    pub(super) doc: DocId,
    tags: String,
    step: SaveStep,
    should_reveal: bool,
}

impl Draft {
    pub(super) fn of(document: &Document) -> Draft {
        Draft {
            doc: document.id,
            tags: text_of(&document.tags),
            step: SaveStep::Typing,
            should_reveal: true,
        }
    }
}

/// Whether the form stays open. It closes when its document is gone or its save went through, and
/// a refused save keeps it open to show the refusal.
pub(super) fn follow(draft: &mut Draft, library: &Library) -> bool {
    let is_listed = library
        .catalogue
        .ready()
        .is_some_and(|catalogue| catalogue.document(draft.doc).is_some());
    if !is_listed {
        return false;
    }
    if draft.step == SaveStep::Sent && !library.busy.contains_key(&draft.doc) {
        if !library.failures.contains_key(&draft.doc) {
            return false;
        }
        draft.step = SaveStep::Refused;
        draft.should_reveal = true;
    }
    true
}

/// Returns true when the person gave the form up, so the caller closes it.
pub(super) fn form(
    ui: &mut egui::Ui,
    document: &Document,
    draft: &mut Draft,
    shared: &Shared,
    intents: &mut Vec<Intent>,
) -> bool {
    let top = ui.cursor().top();
    let is_sent = draft.step == SaveStep::Sent;
    ui.add_enabled_ui(!is_sent, |ui| {
        caption(ui, TAGS);
        TextInput::new(TAGS, &mut draft.tags)
            .id_salt("library_tags")
            .placeholder(TAGS_HINT)
            .show(ui);
    });
    let edit = DocumentTagsEdit::toward(document, &list_of(&draft.tags));
    ui.add_space(space::SM);
    let mut is_cancelled = false;
    ui.horizontal(|ui| {
        let can_save = !edit.is_empty() && !shared.ingest.is_running() && !is_sent;
        let save = Button::primary(SAVE).loading(is_sent);
        if ui.add_enabled(can_save, save).clicked() {
            draft.step = SaveStep::Sent;
            intents.push(Intent::SetDocumentTags(edit));
        }
        let cancel = Button::secondary(CANCEL);
        is_cancelled = ui.add_enabled(!is_sent, cancel).clicked();
    });
    if draft.step == SaveStep::Refused
        && let Some(failure) = shared.library.failures.get(&document.id)
    {
        ui.add_space(space::SM);
        Notice::error(&failure.hint).show(ui);
    }
    reveal(ui, top, &mut draft.should_reveal);
    is_cancelled
}
