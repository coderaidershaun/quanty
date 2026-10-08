//! One document of a media: what is known about it, and the buttons that act on it.

use eframe::egui;

use super::{Form, Local, can_edit, tags_form};
use crate::contract::{Document, Intent};
use crate::panels::PanelCx;
use crate::panels::labels::TAGS;
use crate::theme::{Icon, TextRole, Tone, color, space};
use crate::widgets::{Badge, Button, Card, ControlSize};

const NOT_WHOLE: &str = "The ingest of this document did not finish, so a search may miss parts of it. Ingest it again to finish it.";
const NO_TAGS: &str = "No tags";
const COPY_ID: &str = "Copy id";
const READ: &str = "Read";
const EDIT_TAGS: &str = "Edit tags";
const READ_DISABLED: &str = "The document's folder was not found, so its pages cannot be shown.";

pub(super) fn show(
    ui: &mut egui::Ui,
    document: &Document,
    local: &mut Local,
    cx: &mut PanelCx<'_>,
) {
    let is_edited = matches!(&local.form, Some(Form::Tags(draft)) if draft.doc == document.id);
    Card::new().show(ui, |ui| {
        ui.label(TextRole::BodyStrong.rich(document.name().label()));
        ui.label(TextRole::Small.rich(facts(document)));
        if document.ingested_items.is_none() {
            ui.label(
                TextRole::Small
                    .rich(NOT_WHOLE)
                    .color(Tone::Warning.swatch().text),
            );
        }
        ui.add_space(space::XS);
        match &mut local.form {
            Some(Form::Tags(draft)) if draft.doc == document.id => {
                if tags_form::form(ui, document, draft, cx.shared, cx.intents) {
                    local.form = None;
                }
            }
            _ => labels(ui, document),
        }
        ui.add_space(space::SM);
        buttons(ui, document, is_edited, local, cx);
    });
}

fn facts(document: &Document) -> String {
    match document.pages {
        Some(1) => format!("1 page · {}", document.items),
        Some(pages) => format!("{pages} pages · {}", document.items),
        None => document.items.to_string(),
    }
}

/// The document's own tags. The labels of its media are shown on the media.
fn labels(ui: &mut egui::Ui, document: &Document) {
    ui.horizontal_wrapped(|ui| {
        ui.label(TextRole::Label.rich(TAGS).color(color::TEXT_MUTED));
        if document.tags.is_empty() {
            ui.label(TextRole::Body.rich(NO_TAGS).color(color::TEXT_MUTED));
        }
        for tag in &document.tags {
            ui.add(Badge::new(tag));
        }
    });
}

fn buttons(
    ui: &mut egui::Ui,
    document: &Document,
    is_edited: bool,
    local: &mut Local,
    cx: &mut PanelCx<'_>,
) {
    ui.horizontal(|ui| {
        let copy = Button::secondary(COPY_ID)
            .icon(Icon::COPY)
            .size(ControlSize::Small);
        if ui.add(copy).clicked() {
            cx.intents.push(Intent::CopyText(document.id.0.to_string()));
        }
        let read = Button::secondary(READ)
            .icon(Icon::OPEN)
            .size(ControlSize::Small);
        let read = ui
            .add_enabled(document.folder.is_some(), read)
            .on_disabled_hover_text(READ_DISABLED);
        if read.clicked() {
            cx.intents.push(Intent::OpenSource {
                doc: document.id,
                page: 1,
                piece: None,
            });
        }
        if !is_edited {
            let edit_tags = Button::secondary(EDIT_TAGS)
                .icon(Icon::EDIT)
                .size(ControlSize::Small);
            if ui.add_enabled(can_edit(cx.shared), edit_tags).clicked() {
                local.form = Some(Form::Tags(tags_form::Draft::of(document)));
            }
        }
    });
}
