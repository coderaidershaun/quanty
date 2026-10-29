//! The top of the source panel: its title with the maximise and Open in Library buttons, or the
//! Close button of the reader, the media and document pickers, and the page pager.

mod pager;
mod pickers;
mod rows;

use eframe::egui;

use super::{Host, row};
use crate::panels::{PanelCx, PanelWidth};
use crate::theme::{self, Icon, TextRole};
use crate::widgets::{Button, ControlSize, section_header};
use contract::{Intent, Panel};
use pager::PagerText;
use pickers::Lists;
use rows::{Parts, Rows};

const TITLE: &str = "Source in Context";
const OPEN_IN_LIBRARY: &str = "Open in Library";
const CLOSE_READER: &str = "Close reader";
const OPEN_IN_LIBRARY_OFF: &str = "The library does not list this document yet.";
const CHOOSE_THE_DOCUMENT: &str = "Choose the document";
/// The part of a row that the media picker takes when the document picker is beside it.
const MEDIA_SHARE: f32 = 0.5;

#[derive(Debug, Default)]
pub(super) struct Cache {
    lists: Lists,
    pager: PagerText,
    /// Whether a folded header shows the two pickers.
    is_choosing: bool,
}

pub(super) fn show(
    ui: &mut egui::Ui,
    cache: &mut Cache,
    cx: &mut PanelCx<'_>,
    width: PanelWidth,
    host: Host,
) {
    let shared = cx.shared;
    cache.lists.refresh(shared);
    let size = width.control_size();
    let rows = Rows::pick(width, &measure(ui, cache, cx, size, host));
    let [media, document] = cache.lists.pickers(shared);
    let (mut opened_media, mut opened_document, mut turn) = (None, None, None);
    match rows {
        Rows::Three => {
            section_header(ui, TITLE, |ui| actions(ui, cx, size, host));
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
                    actions(ui, cx, size, host);
                    cache.pager.show(ui, &shared.source, size)
                })
            });
            row::show(ui, |ui| {
                opened_media = media.show(ui, MEDIA_SHARE * ui.available_width(), size);
                opened_document = document.show(ui, ui.available_width(), size);
            });
        }
        Rows::One => {
            turn = row::show(ui, |ui| {
                ui.label(TextRole::Heading.rich(ui, TITLE));
                let gap = ui.spacing().item_spacing.x;
                let left =
                    ui.available_width() - cache.pager.width() - actions_width(cx, size, gap, host);
                // The gap after the title is spent already, so three are left: between the
                // pickers, before the pager and before the buttons.
                let picker = (left - 3.0 * gap) / 2.0;
                opened_media = media.show(ui, picker, size);
                opened_document = document.show(ui, picker, size);
                let step = cache.pager.show(ui, &shared.source, size);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    actions(ui, cx, size, host);
                });
                step
            });
        }
        Rows::Four | Rows::Folded => {
            // A row takes its height from the control height of the style, so that height is
            // raised to the height of these controls.
            ui.scope(|ui| {
                ui.spacing_mut().interact_size.y = size.height();
                section_header(ui, TITLE, |ui| {
                    actions(ui, cx, size, host);
                    if rows == Rows::Folded {
                        choose_the_document(ui, &mut cache.is_choosing, size);
                    }
                });
                if rows == Rows::Four || cache.is_choosing {
                    let room = ui.available_width();
                    opened_media = media.show(ui, room, size);
                    opened_document = document.show(ui, room, size);
                }
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

fn measure(ui: &egui::Ui, cache: &Cache, cx: &PanelCx<'_>, size: ControlSize, host: Host) -> Parts {
    let gap = ui.spacing().item_spacing;
    let text = theme::palette(ui.ctx()).text;
    Parts {
        room: ui.available_size(),
        gap,
        size,
        title: TextRole::Heading.galley(ui, TITLE, text).size().x,
        pager: cache.pager.width(),
        actions: actions_width(cx, size, gap.x, host),
    }
}

/// How wide the buttons of `actions` are together.
fn actions_width(cx: &PanelCx<'_>, size: ControlSize, gap: f32, host: Host) -> f32 {
    let count = if host == Host::Ask && cx.shared.source.target.is_some() {
        2.0
    } else {
        1.0
    };
    count * size.height() + (count - 1.0) * gap
}

/// The button of a folded header that shows or hides the two pickers.
fn choose_the_document(ui: &mut egui::Ui, is_choosing: &mut bool, size: ControlSize) {
    let button = Button::icon_only(Icon::DOCUMENT, CHOOSE_THE_DOCUMENT)
        .size(size)
        .selected(*is_choosing);
    if ui.add(button).clicked() {
        *is_choosing = !*is_choosing;
    }
}

/// In the Library, the one button that closes the reader. In the Ask tab, Maximise at the right
/// end, and beside it the button that opens the shown document in the Library. That button is drawn
/// whenever the panel has a page, also one that is loading or failed.
fn actions(ui: &mut egui::Ui, cx: &mut PanelCx<'_>, size: ControlSize, host: Host) {
    if host == Host::Library {
        if ui
            .add(Button::icon_only(Icon::CLOSE, CLOSE_READER).size(size))
            .clicked()
        {
            cx.intents.push(Intent::CloseReader);
        }
        return;
    }
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
