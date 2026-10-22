//! The Code, Formulas, Figures and Tables views of a document: every item of that kind in the
//! whole document, in reading order, each drawn as the Source panel draws it, with its page and a
//! button that reads it there.

use eframe::egui;

use crate::contract::{
    ChapterContents, ChapterItem, DocId, DocumentView, Failure, FailureKind, Intent, Loadable,
    PieceKind,
};
use crate::panels::PanelCx;
use crate::panels::heights::Heights;
use crate::panels::labels::TRY_AGAIN;
use crate::panels::source::{kind_name, piece_body};
use crate::theme::{Icon, Kind, TextRole, color};
use crate::widgets::{Card, Placeholder};

const LOADING: &str = "Reading the document";
const NOT_FOUND: &str = "The pages of this document were not found";
const FAILED: &str = "The pages of this document could not be read";
const READ_ON: &str = "Read on";

/// What a card is taken to be high until it has been drawn once.
const CARD_ESTIMATE: f32 = 6.0 * TextRole::Body.line_height();

pub(super) fn show(ui: &mut egui::Ui, view: DocumentView, cx: &mut PanelCx<'_>) {
    let Some(kind) = view.piece_kind() else {
        return;
    };
    let shared = cx.shared;
    let contents = &shared.contents;
    if !contents.is_for(shared.shelf.document()) {
        Placeholder::loading(LOADING).show(ui);
        return;
    }
    match &contents.items {
        Loadable::Idle | Loadable::Loading => {
            Placeholder::loading(LOADING).show(ui);
        }
        Loadable::Failed(failure) => failed(ui, failure, view, cx.intents),
        Loadable::Ready(chapter) if chapter.of_kind(kind).next().is_none() => {
            let (icon, title) = nothing_of(view);
            Placeholder::empty(icon, title).show(ui);
        }
        Loadable::Ready(chapter) => list(ui, chapter, view, kind, cx),
    }
}

fn nothing_of(view: DocumentView) -> (Icon, &'static str) {
    match view {
        DocumentView::Formulas => (Icon::FORMULA, "No formulas in this document"),
        DocumentView::Figures => (Icon::FIGURE, "No figures in this document"),
        DocumentView::Tables => (Icon::TABLE, "No tables in this document"),
        DocumentView::Code => (Icon::CODE, "No code listings in this document"),
        DocumentView::Overview | DocumentView::Insights | DocumentView::Concepts => {
            (Icon::DOCUMENT, view.label())
        }
    }
}

fn failed(ui: &mut egui::Ui, failure: &Failure, view: DocumentView, intents: &mut Vec<Intent>) {
    let title = match failure.kind {
        FailureKind::SourceMissing => NOT_FOUND,
        _ => FAILED,
    };
    let shown = Placeholder::error(title)
        .hint(&failure.hint)
        .action(TRY_AGAIN)
        .show(ui);
    shown.response.on_hover_text(&failure.detail);
    if shown.action_clicked {
        intents.push(Intent::ShowDocumentView(view));
    }
}

/// The heights of the cards are kept in egui's memory for each document and view, so a card out of
/// view is only given the room it had and is not laid out.
fn list(
    ui: &mut egui::Ui,
    chapter: &ChapterContents,
    view: DocumentView,
    kind: PieceKind,
    cx: &mut PanelCx<'_>,
) {
    let salt = ("library contents", chapter.doc, view);
    let memory = egui::Id::new(salt);
    let mut kept: Heights = ui
        .data_mut(|data| data.remove_temp(memory))
        .unwrap_or_default();
    let mut heights = Heights::take(&mut kept, CARD_ESTIMATE);
    egui::ScrollArea::vertical()
        .id_salt(salt)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (index, item) in chapter.of_kind(kind).enumerate() {
                heights.show(ui, index, |ui| card(ui, chapter.doc, item, cx));
            }
        });
    ui.data_mut(|data| data.insert_temp(memory, heights));
}

fn card(ui: &mut egui::Ui, doc: DocId, item: &ChapterItem, cx: &mut PanelCx<'_>) {
    let piece = &item.piece;
    let pages = item.page_label();
    let read = format!("{READ_ON} {pages}");
    let tag = piece
        .label
        .as_deref()
        .unwrap_or_else(|| kind_name(piece.kind));
    let shown = Card::new()
        .tag(Kind::from(piece.kind), tag)
        .action(Icon::OPEN, &read)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(TextRole::Small.rich(&pages).color(color::TEXT_MUTED));
            let mut body = piece.clone();
            if let Some(metadata) = &item.metadata {
                ui.label(TextRole::BodyStrong.rich(&metadata.title));
                ui.label(TextRole::Body.rich(&metadata.description));
                if body
                    .name
                    .as_deref()
                    .is_some_and(|name| name == metadata.title || name == metadata.description)
                {
                    body.name = None;
                }
                if body.caption.as_deref().is_some_and(|caption| {
                    caption == metadata.title || caption == metadata.description
                }) {
                    body.caption = None;
                }
            }
            piece_body(ui, &body, cx.media, cx.intents);
        });
    if shown.action_clicked {
        cx.intents.push(Intent::OpenSource {
            doc,
            page: item.page,
            piece: Some(piece.number),
        });
    }
}
