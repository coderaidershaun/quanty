//! The Concepts view of a document: the concepts its ingest found, the most mentioned first, each
//! with its one-line definition, read from the graph.

use eframe::egui;

use super::contents::export_row;
use super::width;
use crate::panels::PanelCx;
use crate::panels::heights::Heights;
use crate::panels::labels::TRY_AGAIN;
use crate::theme::{self, Icon, TextRole, space};
use crate::widgets::Placeholder;
use contract::{DocId, DocumentView, Failure, Intent, Loadable, PageConcept};

const LOADING: &str = "Finding the concepts of this document";
const FAILED: &str = "The concepts could not be read";
const NONE: &str = "No concepts were found in this document";

/// What a row is taken to be high until it has been drawn once.
const ROW_ESTIMATE: f32 = 3.0 * TextRole::Body.line_height();

pub(super) fn show(ui: &mut egui::Ui, cx: &mut PanelCx<'_>) {
    let shared = cx.shared;
    let contents = &shared.contents;
    let Some(doc) = shared
        .shelf
        .document()
        .filter(|doc| contents.is_for(Some(*doc)))
    else {
        Placeholder::loading(LOADING).show(ui);
        return;
    };
    match &contents.concepts {
        Loadable::Idle | Loadable::Loading => {
            Placeholder::loading(LOADING).show(ui);
        }
        Loadable::Failed(failure) => failed(ui, failure, cx.intents),
        Loadable::Ready(list) if list.is_empty() => {
            Placeholder::empty(Icon::CONCEPT, NONE).show(ui);
        }
        Loadable::Ready(list) => {
            export_row(ui, cx, doc, DocumentView::Concepts);
            rows(ui, doc, list);
        }
    }
}

fn failed(ui: &mut egui::Ui, failure: &Failure, intents: &mut Vec<Intent>) {
    let shown = Placeholder::error(FAILED)
        .hint(&failure.hint)
        .action(TRY_AGAIN)
        .action_size(width(ui).control_size())
        .show(ui);
    shown.response.on_hover_text(&failure.detail);
    if shown.action_clicked {
        intents.push(Intent::ShowDocumentView(DocumentView::Concepts));
    }
}

/// The heights of the rows are kept in egui's memory for each document, so a row out of view is
/// only given the room it had and is not laid out.
fn rows(ui: &mut egui::Ui, doc: DocId, list: &[PageConcept]) {
    let salt = ("library concepts", doc);
    let memory = egui::Id::new(salt);
    let mut kept: Heights = ui
        .data_mut(|data| data.remove_temp(memory))
        .unwrap_or_default();
    let mut heights = Heights::take(&mut kept, ROW_ESTIMATE);
    egui::ScrollArea::vertical()
        .id_salt(salt)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (index, concept) in list.iter().enumerate() {
                heights.show(ui, index, |ui| row(ui, concept));
            }
        });
    ui.data_mut(|data| data.insert_temp(memory, heights));
}

fn row(ui: &mut egui::Ui, concept: &PageConcept) {
    ui.label(TextRole::BodyStrong.rich(ui, &concept.name));
    if !concept.definition.is_empty() {
        let definition_color = theme::palette(ui.ctx()).text_secondary;
        ui.label(
            TextRole::Body
                .rich(ui, &concept.definition)
                .color(definition_color),
        );
    }
    ui.add_space(space::SM);
}
