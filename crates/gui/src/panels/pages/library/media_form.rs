//! The form that changes the category, the authors and the tags of a stored media, and how its
//! save went. The title names the media and is not in the form.

use eframe::egui;

use super::{SaveStep, reveal};
use crate::contract::{Failure, Intent, Media, is_same_name};
use crate::panels::media_card::{
    self, FormSetup, MediaFields, Pressed, Purpose, Save, would_change,
};
use crate::state::{Library, MediaEditing, Shared};

#[derive(Debug)]
pub(super) struct Draft {
    /// Its title is the library's own. It names the media, so it is sent unchanged.
    fields: MediaFields,
    step: SaveStep,
    should_reveal: bool,
}

impl Draft {
    pub(super) fn of(media: &Media) -> Draft {
        Draft {
            fields: MediaFields::of(media),
            step: SaveStep::Typing,
            should_reveal: true,
        }
    }

    /// The draft keeps the title exactly as the library has it, so an exact match finds its card.
    pub(super) fn is_for(&self, title: &str) -> bool {
        self.fields.title == title
    }
}

/// Whether the form stays open. It closes when its media is no longer listed or its save went
/// through, and a refused save keeps it open to show the refusal.
pub(super) fn follow(draft: &mut Draft, library: &Library) -> bool {
    let is_listed = library
        .catalogue
        .ready()
        .is_some_and(|catalogue| catalogue.media_titled(&draft.fields.title).is_some());
    if !is_listed {
        return false;
    }
    if draft.step == SaveStep::Sent && !library.media_edit.is_saving() {
        if refusal_of(draft, library).is_none() {
            return false;
        }
        draft.step = SaveStep::Refused;
        draft.should_reveal = true;
    }
    true
}

/// The app keeps one failed edit for all media, and another tab can send an edit of another
/// media, so the failure counts only when it names the media of this draft.
fn refusal_of<'a>(draft: &Draft, library: &'a Library) -> Option<&'a Failure> {
    match &library.media_edit {
        MediaEditing::Failed { edit, failure }
            if is_same_name(&edit.title, &draft.fields.title) =>
        {
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
    let edit = draft.fields.to_edit();
    let save = if draft.step == SaveStep::Sent {
        Save::Sent
    } else if would_change(&edit, media) && shared.can_edit_media() {
        Save::On
    } else {
        Save::Off
    };
    let refusal = match draft.step {
        SaveStep::Refused => refusal_of(draft, &shared.library),
        SaveStep::Typing | SaveStep::Sent => None,
    };
    let setup = FormSetup {
        id_salt: "library_media",
        purpose: Purpose::Edit,
        save,
        refusal,
    };
    let pressed = media_card::media_form(ui, &mut draft.fields, setup);
    if pressed == Some(Pressed::Save) {
        draft.step = SaveStep::Sent;
        intents.push(Intent::EditMedia(draft.fields.to_edit()));
    }
    reveal(ui, top, &mut draft.should_reveal);
    pressed == Some(Pressed::Cancel)
}
