//! The top of the source panel: its title with the maximise and Open in Library buttons, the media
//! and document pickers, and the page pager.

mod pager;
mod pickers;

use eframe::egui;

use super::{PanelWidth, row};
use crate::panels::PanelCx;
use crate::theme::Icon;
use crate::widgets::{Button, ControlSize, section_header};
use contract::{Intent, Panel};
use pager::PagerText;
use pickers::Lists;

const TITLE: &str = "Source in Context";
const OPEN_IN_LIBRARY: &str = "Open in Library";
const OPEN_IN_LIBRARY_OFF: &str = "The library does not list this document yet.";
/// The panel's inner height from which a roomy header has three rows. Below it the pager moves up
/// beside the title and the two pickers share a row, so the page keeps its room.
const FOLD_BELOW: f32 = 440.0;
/// The part of a row that the media picker takes when the document picker is beside it.
const MEDIA_SHARE: f32 = 0.5;

#[derive(Debug, Default)]
pub(super) struct Cache {
    lists: Lists,
    pager: PagerText,
}

/// How the header is laid out: the title with its buttons, the media picker, the document picker
/// and the pager, in two, three or four rows. A narrow panel has no room for two controls side by
/// side, so each of its rows holds one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rows {
    Two,
    Three,
    Four,
}

pub(super) fn show(ui: &mut egui::Ui, cache: &mut Cache, cx: &mut PanelCx<'_>, width: PanelWidth) {
    let shared = cx.shared;
    cache.lists.refresh(shared);
    let rows = match width {
        PanelWidth::Narrow => Rows::Four,
        PanelWidth::Roomy if ui.available_height() >= FOLD_BELOW => Rows::Three,
        PanelWidth::Roomy => Rows::Two,
    };
    let size = width.control_size();
    let [media, document] = cache.lists.pickers(shared);
    let (mut opened_media, mut opened_document, mut turn) = (None, None, None);
    match rows {
        Rows::Three => {
            section_header(ui, TITLE, |ui| actions(ui, cx, size));
            let room = ui.available_width();
            opened_media = media.show(ui, room, size);
            row::show(ui, |ui| {
                let gap = ui.spacing().item_spacing.x;
                let rest = ui.available_width() - gap - cache.pager.width();
                opened_document = document.show(ui, rest.max(0.0), size);
                turn = cache.pager.show(ui, &shared.source, size);
            });
        }
        Rows::Two => {
            turn = row::show(ui, |ui| {
                section_header(ui, TITLE, |ui| {
                    actions(ui, cx, size);
                    cache.pager.show(ui, &shared.source, size)
                })
            });
            row::show(ui, |ui| {
                opened_media = media.show(ui, MEDIA_SHARE * ui.available_width(), size);
                opened_document = document.show(ui, ui.available_width(), size);
            });
        }
        Rows::Four => {
            // A row takes its height from the control height of the style, so that height is
            // raised to the height of these controls.
            ui.scope(|ui| {
                ui.spacing_mut().interact_size.y = size.height();
                section_header(ui, TITLE, |ui| actions(ui, cx, size));
                let room = ui.available_width();
                opened_media = media.show(ui, room, size);
                opened_document = document.show(ui, room, size);
                turn = row::show(ui, |ui| cache.pager.show(ui, &shared.source, size));
            });
        }
    }
    for doc in [opened_media, opened_document].into_iter().flatten() {
        cx.intents.push(Intent::OpenSource {
            doc,
            page: 1,
            piece: None,
        });
    }
    if let Some(delta) = turn {
        cx.intents.push(Intent::TurnPage(delta));
    }
}

/// Maximise at the right end, and beside it the button that opens the shown document in the
/// Library. That button is drawn whenever the panel has a page, also one that is loading or failed.
fn actions(ui: &mut egui::Ui, cx: &mut PanelCx<'_>, size: ControlSize) {
    cx.show_maximise_toggle(ui, Panel::Source, size);
    let Some(target) = cx.shared.source.target else {
        return;
    };
    let is_listed = cx
        .shared
        .library
        .catalogue
        .ready()
        .is_some_and(|catalogue| catalogue.document(target.doc).is_some());
    let open = ui
        .add_enabled(
            is_listed,
            Button::icon_only(Icon::LIBRARY, OPEN_IN_LIBRARY).size(size),
        )
        .on_disabled_hover_text(OPEN_IN_LIBRARY_OFF);
    if open.clicked() {
        cx.intents.push(Intent::OpenDocument(target.doc));
    }
}
