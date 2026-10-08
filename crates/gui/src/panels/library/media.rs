//! One media of the library as a card: its title, category, authors and tags, the pencil that
//! opens its form, and the cards of its documents.

use eframe::egui;

use super::{Form, Local, can_edit, document, media_form};
use crate::contract::Media;
use crate::panels::PanelCx;
use crate::panels::media_card::{self, NO_MEDIA, Pencil};
use crate::theme::{TextRole, color, space};
use crate::widgets::Card;

const NO_DOCUMENT: &str = "No document yet. Add one on the Ingest tab.";

pub(super) fn show(ui: &mut egui::Ui, media: &Media, local: &mut Local, cx: &mut PanelCx<'_>) {
    Card::new().show(ui, |ui| {
        header(ui, media, local, cx);
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
    if media_card::title_line(ui, title, media.category, pencil) {
        local.form = Some(Form::Media(media_form::Draft::of(media)));
    }
}
