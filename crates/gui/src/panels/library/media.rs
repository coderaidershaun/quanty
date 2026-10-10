//! One media of the library as a card: its title, category, authors and tags, the pencil that
//! opens its form, its Delete button, and the cards of its documents.

use eframe::egui;

use super::{Form, Local, can_edit, delete, document, media_form};
use crate::contract::{Failure, Media, is_same_name};
use crate::panels::PanelCx;
use crate::panels::media_card::{self, NO_MEDIA, Pencil};
use crate::state::MediaDelete;
use crate::theme::{TextRole, color, space};
use crate::widgets::{Card, Notice};

const NO_DOCUMENT: &str = "No document yet. Add one on the Ingest tab.";

pub(super) fn show(ui: &mut egui::Ui, media: &Media, local: &mut Local, cx: &mut PanelCx<'_>) {
    Card::new().show(ui, |ui| {
        header(ui, media, local, cx);
        let media_delete = &cx.shared.library.media_delete;
        if let Some(failure) = media
            .title
            .as_deref()
            .and_then(|title| failed_delete(title, media_delete))
        {
            ui.add_space(space::XS);
            Notice::error(&failure.hint).show(ui);
        }
        ui.add_space(space::XS);
        // The documents of no media have no labels to show and no form to open. Each document
        // shows only its own tags, so the tags of the media are shown once, here.
        match (media.title.as_deref(), &mut local.form) {
            (Some(title), Some(Form::Media(draft))) if draft.is_for(title) => {
                if media_form::form(ui, media, draft, cx.shared, cx.intents) {
                    local.form = None;
                }
            }
            (Some(_), _) => media_card::labels_line(ui, media),
            (None, _) => {}
        }
        ui.add_space(space::SM);
        if media.documents.is_empty() {
            ui.label(TextRole::Small.rich(NO_DOCUMENT).color(color::TEXT_MUTED));
        }
        for (index, listed) in media.documents.iter().enumerate() {
            if index > 0 {
                ui.add_space(space::SM);
            }
            document::show(ui, listed, local, cx);
        }
    });
}

fn header(ui: &mut egui::Ui, media: &Media, local: &mut Local, cx: &PanelCx<'_>) {
    let Some(title) = media.title.as_deref() else {
        ui.label(TextRole::BodyStrong.rich(NO_MEDIA));
        return;
    };
    let is_open = matches!(&local.form, Some(Form::Media(draft)) if draft.is_for(title));
    let pencil = if is_open {
        Pencil::Hidden
    } else if can_edit(cx.shared) {
        Pencil::On
    } else {
        Pencil::Off
    };
    let library = &cx.shared.library;
    let is_busy = is_being_deleted(title, &library.media_delete);
    let name = format!("Delete media {title}");
    let (is_delete_pressed, is_pencil_pressed) =
        media_card::title_line(ui, title, media.category, |ui| {
            let is_delete_pressed = delete::button(ui, &name, is_busy, cx.shared);
            (is_delete_pressed, media_card::pencil(ui, title, pencil))
        });
    if is_delete_pressed && let Some(catalogue) = library.catalogue.ready() {
        local.question = Some(delete::Question::of_media(title, catalogue));
    }
    if is_pencil_pressed {
        local.form = Some(Form::Media(media_form::Draft::of(media)));
    }
}

/// The delete of the media that `title` names is on its way.
fn is_being_deleted(title: &str, media_delete: &MediaDelete) -> bool {
    match media_delete {
        MediaDelete::Deleting {
            title: on_its_way, ..
        } => is_same_name(on_its_way, title),
        MediaDelete::Idle | MediaDelete::Failed { .. } => false,
    }
}

/// Why the last delete of the media that `title` names failed.
fn failed_delete<'a>(title: &str, media_delete: &'a MediaDelete) -> Option<&'a Failure> {
    match media_delete {
        MediaDelete::Failed {
            title: failed,
            failure,
        } if is_same_name(failed, title) => Some(failure),
        _ => None,
    }
}
