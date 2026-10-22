//! The page of the open document: its media and name, its facts, the strip of its views, and the
//! body of the view that is shown.

use eframe::egui;

use super::{Local, concepts, contents, insights, overview};
use crate::panels::PanelCx;
use crate::panels::labels::NO_MEDIA;
use crate::panels::pages::media_card;
use crate::state::Contents;
use crate::theme::{TextRole, Tone, space};
use crate::widgets::{Tab, TabStrip};
use contract::{Document, DocumentView, Intent, Media};

const NOT_WHOLE: &str = "The ingest of this document did not finish, so a search may miss parts of it. Ingest it again to finish it.";

pub(super) fn show(
    ui: &mut egui::Ui,
    media: &Media,
    document: &Document,
    local: &mut Local,
    cx: &mut PanelCx<'_>,
) {
    header(ui, media, document);
    ui.add_space(space::SM);
    strip(ui, document, cx);
    ui.add_space(space::MD);
    let view = cx.shared.shelf.view;
    match view {
        DocumentView::Overview => overview::show(ui, document, local, cx),
        DocumentView::Insights => insights::show(ui, cx),
        DocumentView::Code
        | DocumentView::Formulas
        | DocumentView::Figures
        | DocumentView::Tables => contents::show(ui, view, cx),
        DocumentView::Concepts => concepts::show(ui, cx),
    }
}

fn header(ui: &mut egui::Ui, media: &Media, document: &Document) {
    match media.title.as_deref() {
        Some(title) => media_card::title_line(ui, title, media.category, |_| ()),
        None => {
            ui.label(TextRole::BodyStrong.rich(NO_MEDIA));
        }
    }
    ui.label(TextRole::Heading.rich(document.name().label()));
    ui.label(TextRole::Small.rich(facts(document)));
    if document.ingested_items.is_none() {
        ui.label(
            TextRole::Small
                .rich(NOT_WHOLE)
                .color(Tone::Warning.swatch().text),
        );
    }
}

fn facts(document: &Document) -> String {
    match document.pages {
        Some(1) => format!("1 page, {}", document.items),
        Some(pages) => format!("{pages} pages, {}", document.items),
        None => document.items.to_string(),
    }
}

fn strip(ui: &mut egui::Ui, document: &Document, cx: &mut PanelCx<'_>) {
    let tabs = DocumentView::ALL.map(|view| match count_of(view, document, &cx.shared.contents) {
        Some(count) => Tab::new(view.label()).count(count),
        None => Tab::new(view.label()),
    });
    let active = DocumentView::ALL
        .iter()
        .position(|view| *view == cx.shared.shelf.view)
        .unwrap_or_default();
    if let Some(index) = TabStrip::new(&tabs, active).show(ui)
        && let Some(view) = DocumentView::ALL.get(index)
    {
        cx.intents.push(Intent::ShowDocumentView(*view));
    }
}

/// What each view counts: what the views read for this document, once they have read it, and else
/// what the catalogue counts for the views that list pieces.
fn count_of(view: DocumentView, document: &Document, contents: &Contents) -> Option<usize> {
    if contents.is_for(Some(document.id)) {
        let read = match view.piece_kind() {
            Some(kind) => contents
                .items
                .ready()
                .map(|chapter| chapter.of_kind(kind).count()),
            None if view == DocumentView::Concepts => contents.concepts.ready().map(Vec::len),
            None => None,
        };
        if read.is_some() {
            return read;
        }
    }
    let items = &document.items;
    let count = match view {
        DocumentView::Code => items.code,
        DocumentView::Formulas => items.formulas,
        DocumentView::Figures => items.figures,
        DocumentView::Tables => items.tables,
        DocumentView::Overview | DocumentView::Insights | DocumentView::Concepts => return None,
    };
    Some(usize::try_from(count).unwrap_or(usize::MAX))
}
