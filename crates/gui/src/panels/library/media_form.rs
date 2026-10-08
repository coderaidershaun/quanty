//! The form that changes the category, the authors and the tags of a stored media, and how its
//! save went. The title names the media and is not in the form.

use eframe::egui;

use super::{CANCEL, SAVE, SaveStep, can_send_media_edit, reveal};
use crate::contract::{Category, Failure, Intent, Media, MediaEdit, is_same_title};
use crate::panels::labels::{AUTHORS, LIST_PLACEHOLDER, TAGS, caption, list_of, text_of};
use crate::state::{Library, MediaEditing, Shared};
use crate::theme::space;
use crate::widgets::{Button, Chip, Notice, TextInput};

const CATEGORY: &str = "Category";

#[derive(Debug)]
pub(super) struct Draft {
    /// The title as the library has it. It names the media, so it is sent unchanged.
    title: String,
    category: Category,
    authors: String,
    tags: String,
    step: SaveStep,
    should_reveal: bool,
}

impl Draft {
    pub(super) fn of(title: &str, media: &Media) -> Draft {
        Draft {
            title: title.to_owned(),
            category: media.category,
            authors: text_of(&media.authors),
            tags: text_of(&media.tags),
            step: SaveStep::Typing,
            should_reveal: true,
        }
    }

    /// The draft keeps the title exactly as the library has it, so an exact match finds its card.
    pub(super) fn is_for(&self, title: &str) -> bool {
        self.title == title
    }
}

/// Whether the form stays open. It closes when its media is no longer listed or its save went
/// through, and a refused save keeps it open to show the refusal.
pub(super) fn follow(draft: &mut Draft, library: &Library) -> bool {
    let is_listed = library
        .catalogue
        .ready()
        .is_some_and(|catalogue| catalogue.media_titled(&draft.title).is_some());
    if !is_listed {
        return false;
    }
    if draft.step == SaveStep::Sent && !library.media_edit.is_saving() {
        if refusal(draft, library).is_none() {
            return false;
        }
        draft.step = SaveStep::Refused;
        draft.should_reveal = true;
    }
    true
}

/// The app keeps one failed edit for all media, and another tab can send an edit of another
/// media, so the failure counts only when it names the media of this draft.
fn refusal<'a>(draft: &Draft, library: &'a Library) -> Option<&'a Failure> {
    match &library.media_edit {
        MediaEditing::Failed { edit, failure } if is_same_title(&edit.title, &draft.title) => {
            Some(failure)
        }
        _ => None,
    }
}

/// Returns true when the person gave the form up, so the caller closes it.
pub(super) fn form(
    ui: &mut egui::Ui,
    media: &Media,
    draft: &mut Draft,
    shared: &Shared,
    intents: &mut Vec<Intent>,
) -> bool {
    let top = ui.cursor().top();
    let is_sent = draft.step == SaveStep::Sent;
    ui.add_enabled_ui(!is_sent, |ui| fields(ui, draft));
    let edit = MediaEdit {
        title: draft.title.clone(),
        category: draft.category,
        authors: list_of(&draft.authors),
        tags: list_of(&draft.tags),
    };
    ui.add_space(space::SM);
    let mut is_cancelled = false;
    ui.horizontal(|ui| {
        let can_save = would_change(&edit, media) && can_send_media_edit(shared) && !is_sent;
        let save = Button::primary(SAVE).loading(is_sent);
        if ui.add_enabled(can_save, save).clicked() {
            draft.step = SaveStep::Sent;
            intents.push(Intent::EditMedia(edit));
        }
        let cancel = Button::secondary(CANCEL);
        is_cancelled = ui.add_enabled(!is_sent, cancel).clicked();
    });
    if draft.step == SaveStep::Refused
        && let Some(failure) = refusal(draft, &shared.library)
    {
        ui.add_space(space::SM);
        Notice::error(&failure.hint).show(ui);
    }
    reveal(ui, top, &mut draft.should_reveal);
    is_cancelled
}

fn fields(ui: &mut egui::Ui, draft: &mut Draft) {
    caption(ui, CATEGORY);
    ui.horizontal(|ui| {
        for category in Category::ALL {
            let chip = Chip::plain(category.label()).selected(category == draft.category);
            if ui.add(chip).clicked() {
                draft.category = category;
            }
        }
    });
    caption(ui, AUTHORS);
    TextInput::new(AUTHORS, &mut draft.authors)
        .id_salt("library_media_authors")
        .placeholder(LIST_PLACEHOLDER)
        .show(ui);
    caption(ui, TAGS);
    TextInput::new(TAGS, &mut draft.tags)
        .id_salt("library_media_tags")
        .placeholder(LIST_PLACEHOLDER)
        .show(ui);
}

/// The stores keep the tags in lower case, so "Options" for a stored "options" is no change. A
/// repeated or reordered author or tag counts as a change, and saving it is harmless.
fn would_change(edit: &MediaEdit, media: &Media) -> bool {
    let tags: Vec<String> = edit.tags.iter().map(|tag| tag.to_lowercase()).collect();
    edit.category != media.category || edit.authors != media.authors || tags != media.tags
}
