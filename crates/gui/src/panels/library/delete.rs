//! The question that the sheet asks before a document or a media is deleted, the words it says,
//! and the Delete button of a card.

use eframe::egui;

use super::can_edit;
use crate::contract::{Catalogue, DocId, Document, Intent};
use crate::state::Shared;
use crate::theme::Icon;
use crate::widgets::{Button, Choice, Confirm, ControlSize};

const DELETE: &str = "Delete";
const DELETE_DOCUMENT: &str = "Delete document";
const DELETE_MEDIA: &str = "Delete media";

const DOCUMENT_BODY: &str = "Its passages, formulas, figures and tables go from the library. Its converted pages go from the disk, and so does the copy of its PDF, if quanty keeps one. The PDF you picked from your own disk is not touched. This cannot be undone.";
const MEDIA_WITHOUT_DOCUMENT_BODY: &str =
    "It has no document, so only the media goes. This cannot be undone.";
const MEDIA_WITH_ONE_DOCUMENT_BODY: &str = "Its 1 document goes with it: its passages, formulas, figures and tables go from the library, and its converted pages go from the disk, with the copy of its PDF if quanty keeps one. The PDF you picked from your own disk is not touched. This cannot be undone.";

const OFF_WHILE_INGESTING: &str = "Nothing can be deleted while an ingest runs.";
const OFF_WHILE_UPDATING: &str = "Nothing can be deleted while the library is being updated.";

#[derive(Debug, Clone, PartialEq, Eq)]
enum Target {
    Document(DocId),
    /// The title as the library has it.
    Media(String),
}

/// What the sheet asks. The words are made once, when a Delete button is pressed.
#[derive(Debug)]
pub(super) struct Question {
    target: Target,
    title: String,
    body: String,
}

impl Question {
    pub(super) fn of_document(document: &Document) -> Question {
        Question {
            target: Target::Document(document.id),
            title: format!("Delete the document {}?", document.name().label()),
            body: DOCUMENT_BODY.to_owned(),
        }
    }

    /// Counts the documents of every listed media of that name, because that is what goes.
    pub(super) fn of_media(title: &str, catalogue: &Catalogue) -> Question {
        Question {
            target: Target::Media(title.to_owned()),
            title: format!("Delete the media {title}?"),
            body: media_body(catalogue.documents_of_media(title).count()),
        }
    }

    fn confirm_label(&self) -> &'static str {
        match self.target {
            Target::Document(_) => DELETE_DOCUMENT,
            Target::Media(_) => DELETE_MEDIA,
        }
    }

    fn into_intent(self) -> Intent {
        match self.target {
            Target::Document(doc) => Intent::DeleteDocument(doc),
            Target::Media(title) => Intent::DeleteMedia(title),
        }
    }
}

fn media_body(documents: usize) -> String {
    match documents {
        0 => MEDIA_WITHOUT_DOCUMENT_BODY.to_owned(),
        1 => MEDIA_WITH_ONE_DOCUMENT_BODY.to_owned(),
        count => format!(
            "Its {count} documents go with it: the passages, formulas, figures and tables of each go from the library, and the converted pages of each go from the disk, with the copy of its PDF if quanty keeps one. The PDFs you picked from your own disk are not touched. This cannot be undone."
        ),
    }
}

/// The question is dropped when the library can no longer take a delete, or when what it asks
/// about is not listed any more, so a sheet never asks about something that is gone.
pub(super) fn follow(question: &mut Option<Question>, shared: &Shared) {
    let Some(asked) = question else {
        return;
    };
    let is_listed = shared
        .library
        .catalogue
        .ready()
        .is_some_and(|catalogue| match &asked.target {
            Target::Document(doc) => catalogue.document(*doc).is_some(),
            Target::Media(title) => catalogue.media_titled(title).is_some(),
        });
    if !is_listed || !can_edit(shared) {
        *question = None;
    }
}

/// Shows the sheet while there is a question, and pushes the delete only when the person confirms.
/// Returns true in the frame of the confirm and in no other.
pub(super) fn sheet(
    ctx: &egui::Context,
    question: &mut Option<Question>,
    intents: &mut Vec<Intent>,
) -> bool {
    let Some(asked) = question else {
        return false;
    };
    let choice = Confirm::new(egui::Id::new("library-delete"), &asked.title)
        .body(&asked.body)
        .confirm_label(asked.confirm_label())
        .destructive()
        .show(ctx);
    match choice {
        None => false,
        Some(Choice::Cancelled) => {
            *question = None;
            false
        }
        Some(Choice::Confirmed) => {
            if let Some(asked) = question.take() {
                intents.push(asked.into_intent());
            }
            true
        }
    }
}

/// The Delete button of a card, named after what it deletes. It shows that its own delete is on
/// its way, and it is off, with the reason on hover, while no delete can start. Returns true when
/// it was pressed.
pub(super) fn button(ui: &mut egui::Ui, name: &str, is_busy: bool, shared: &Shared) -> bool {
    let button = Button::secondary(DELETE)
        .icon(Icon::DELETE)
        .size(ControlSize::Small)
        .accessible_name(name)
        .loading(is_busy);
    ui.add_enabled(can_edit(shared), button)
        .on_disabled_hover_text(why_off(shared))
        .clicked()
}

fn why_off(shared: &Shared) -> &'static str {
    if shared.ingest.is_running() {
        OFF_WHILE_INGESTING
    } else {
        OFF_WHILE_UPDATING
    }
}
