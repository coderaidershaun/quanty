//! The Overview of a document: its own tags or the form that changes them, its last failure, and
//! the buttons that act on it.

use eframe::egui;

use super::{Form, Local, can_edit, delete, tags_form, width};
use crate::panels::labels::TAGS;
use crate::panels::{PanelCx, PanelWidth};
use crate::state::Busy;
use crate::theme::{Icon, TextRole, color, space};
use crate::widgets::{Button, Chip, Notice};
use contract::{Document, Intent};

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
    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .show(ui, |ui| {
            match &mut local.form {
                Some(Form::Tags(draft)) if draft.doc == document.id => {
                    if tags_form::form(ui, document, draft, cx.shared, cx.intents) {
                        local.form = None;
                    }
                }
                _ => labels(ui, document),
            }
            // The failure is said above the buttons, so it is in view with the button that was
            // pressed. The open form of the tags says a refused save itself.
            if !is_edited && let Some(failure) = cx.shared.library.failures.get(&document.id) {
                ui.add_space(space::SM);
                Notice::error(&failure.hint).show(ui);
            }
            ui.add_space(space::SM);
            buttons(ui, document, is_edited, local, cx);
        });
}

/// The document's own tags. The labels of its media are shown on the media.
fn labels(ui: &mut egui::Ui, document: &Document) {
    ui.horizontal_wrapped(|ui| {
        ui.label(TextRole::Label.rich(TAGS).color(color::TEXT_MUTED));
        if document.tags.is_empty() {
            ui.label(TextRole::Body.rich(NO_TAGS).color(color::TEXT_MUTED));
        }
        for tag in &document.tags {
            ui.add(Chip::stamp(tag));
        }
    });
}

/// The buttons that act on the document. In a narrow page they are tall enough for a finger and
/// go on to a second line when one line is too short for them.
fn buttons(
    ui: &mut egui::Ui,
    document: &Document,
    is_edited: bool,
    local: &mut Local,
    cx: &mut PanelCx<'_>,
) {
    let panel = width(ui);
    let size = panel.control_size();
    let row = |ui: &mut egui::Ui| {
        let copy = Button::ghost(COPY_ID).icon(Icon::COPY).size(size);
        if ui.add(copy).clicked() {
            cx.intents.push(Intent::CopyText(document.id.0.to_string()));
        }
        let read = Button::ghost(READ).icon(Icon::OPEN).size(size);
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
            let edit_tags = Button::ghost(EDIT_TAGS).icon(Icon::EDIT).size(size);
            if ui.add_enabled(can_edit(cx.shared), edit_tags).clicked() {
                local.form = Some(Form::Tags(tags_form::Draft::of(document)));
            }
        }
        let name = format!("Delete document {}", document.name().label());
        let is_busy = matches!(
            cx.shared.library.busy.get(&document.id),
            Some(Busy::Deleting(_))
        );
        let delete = delete::DeleteButton {
            name: &name,
            is_busy,
            size,
        };
        if delete.show(ui, cx.shared) {
            local.question = Some(delete::Question::of_document(document));
        }
    };
    if panel == PanelWidth::Narrow {
        ui.horizontal_wrapped(row);
    } else {
        ui.horizontal(row);
    }
}
