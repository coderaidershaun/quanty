//! One media of the library as a card: its title, category, authors and tags, the pencil that
//! opens its form, and the cards of its documents.

use eframe::egui;

use super::{Form, Local, can_edit, document, media_form};
use crate::contract::{Category, Media};
use crate::panels::PanelCx;
use crate::theme::{Icon, TextRole, Tone, color, space};
use crate::widgets::{Badge, Button, Card};

const NO_MEDIA: &str = "No media";
const NO_DOCUMENT: &str = "No document yet. Add one on the Ingest tab.";

pub(super) fn show(ui: &mut egui::Ui, media: &Media, local: &mut Local, cx: &mut PanelCx<'_>) {
    Card::new().show(ui, |ui| {
        header(ui, media, local, cx);
        ui.add_space(space::XS);
        // The documents of no media have no labels to show and no form to open.
        match (media.title.as_deref(), &mut local.form) {
            (Some(title), Some(Form::Media(draft))) if draft.is_for(title) => {
                if media_form::form(ui, media, draft, cx.shared, cx.intents) {
                    local.form = None;
                }
            }
            (Some(_), _) => labels(ui, media),
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
    ui.horizontal(|ui| {
        let Some(title) = media.title.as_deref() else {
            ui.label(TextRole::BodyStrong.rich(NO_MEDIA));
            return;
        };
        ui.label(TextRole::BodyStrong.rich(title));
        ui.add(category_badge(media.category));
        let is_open = matches!(&local.form, Some(Form::Media(draft)) if draft.is_for(title));
        if is_open {
            return;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let name = format!("Edit {title}");
            let pencil = Button::icon_only(Icon::EDIT, &name);
            if ui.add_enabled(can_edit(cx.shared), pencil).clicked() {
                local.form = Some(Form::Media(media_form::Draft::of(title, media)));
            }
        });
    });
}

/// Each document shows only its own tags, so the tags of the media are shown once, here.
fn labels(ui: &mut egui::Ui, media: &Media) {
    if media.authors.is_empty() && media.tags.is_empty() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        if !media.authors.is_empty() {
            let authors = media.authors.join(", ");
            ui.label(TextRole::Small.rich(authors).color(color::TEXT_SECONDARY));
        }
        for tag in &media.tags {
            ui.add(Badge::new(tag));
        }
    });
}

fn category_badge(category: Category) -> Badge<'static> {
    let (tone, icon) = match category {
        Category::Book => (Tone::Blue, Icon::BOOK),
        Category::Paper => (Tone::Purple, Icon::DOCUMENT),
        Category::Other => (Tone::Neutral, Icon::FOLDER),
    };
    Badge::new(category.label()).tone(tone).icon(icon)
}
