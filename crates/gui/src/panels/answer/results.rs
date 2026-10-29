//! The list of results, one card for each, with a preview of a fixed height. A preview that is
//! longer than that scrolls inside the card.

use eframe::egui;

use super::listing::{Row, copy_label};
use super::pane::Pane;
use super::phase::AnswerTab;
use crate::media::{code, images, math, rich_text};
use crate::panels::heights::{Heights, Slot};
use crate::theme::{self, Icon, Kind, TextRole, size, space};
use crate::widgets::{Badge, Card, CitationChip, Placeholder};
use contract::{Intent, ItemKind, ResultItem};

const TEXT_PREVIEW: f32 = 3.0 * TextRole::Body.line_height();
/// The size of a figure's picture in the list, in points.
const THUMBNAIL: egui::Vec2 = egui::vec2(144.0, 96.0);
/// This is room for a table's header and two rows. A row is a line of text with the room that a
/// table leaves above and below it.
// SMELL: this repeats the type of a table's header and the room a table leaves around a row,
// which the table keeps to itself, so a change to either must be made here too.
const TABLE_PREVIEW: f32 = (TextRole::Caps.line_height() + 2.0 * space::XS)
    + 2.0 * (TextRole::Body.line_height() + 2.0 * space::XS);
/// What a row is taken to be high until it has been drawn once: a card with the chip line, the
/// reason and three lines of preview.
const ROW_ESTIMATE: f32 =
    2.0 * space::MD + size::CHIP + 2.0 * space::SM + TextRole::Small.line_height() + TEXT_PREVIEW;
/// A code listing shows its first four lines.
const CODE_PREVIEW: f32 = code::height_of(4);
const SCORE_WIDTH: f32 = size::CONTROL_LG;
/// Rounding to whole pixels can move a row by a hair. This is the room that allows for it.
const SLACK: f32 = 1.0;
const SCORE_HELP: &str = "How close the item is to the question. Higher is closer.";

pub(super) fn list(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, tab: AnswerTab) {
    let none_of_the_kind = pane
        .listing
        .and_then(|listing| listing.counts.of(tab))
        .is_some_and(|count| count == 0);
    if none_of_the_kind && let Some((icon, title)) = nothing_of(tab) {
        Placeholder::empty(icon, title).show(ui);
        return;
    }
    // HAZARD: keep this area from animating. An animated area moves its offset a little in each
    // frame, so a request to scroll that is repeated every frame overshoots the row. The stop
    // test in `reveal` is only right while the offset changes once, at the end of a frame.
    egui::ScrollArea::vertical()
        .id_salt((tab, "list", pane.ask.generation))
        .animated(false)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = space::MD;
            ui.add_space(space::MD);
            rows(ui, pane, tab);
            ui.add_space(space::LG);
        });
}

fn nothing_of(tab: AnswerTab) -> Option<(Icon, &'static str)> {
    match tab {
        AnswerTab::Formulas => Some((Kind::Formula.icon(), "No formulas among the results")),
        AnswerTab::Figures => Some((Kind::Figure.icon(), "No figures among the results")),
        AnswerTab::Tables => Some((Kind::Table.icon(), "No tables among the results")),
        AnswerTab::Code => Some((Kind::Code.icon(), "No code among the results")),
        AnswerTab::Answer | AnswerTab::Results => None,
    }
}

fn rows(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, tab: AnswerTab) {
    let (ask, Some(listing)) = (pane.ask, pane.listing) else {
        return;
    };
    let mut heights = Heights::take(&mut pane.view.row_heights, ROW_ESTIMATE);
    // Every numeral stands in a slot as wide as the widest one of the list, so that the badge
    // after it starts at the same place in every row.
    let number_width = ask
        .results()
        .iter()
        .filter(|item| tab.lists(item.kind))
        .map(|item| CitationChip::new(item.number).size(ui).x)
        .fold(0.0, f32::max);
    for (place, (item, row)) in ask.results().iter().zip(&listing.rows).enumerate() {
        if !tab.lists(item.kind) {
            continue;
        }
        let slot = heights.show(ui, place, |ui| card(ui, pane, item, row, number_width));
        if pane.is_selected(item.number) {
            reveal(ui, pane, slot);
        }
    }
    pane.view.row_heights = heights;
}

/// Scrolls the selected row into view, and stops asking in the frame egui would stop moving.
///
/// HAZARD: the two tests below are egui's own rule for "I would still move". A row taller than
/// the view is never fully in view, so any stricter test would ask for ever and pull against
/// the mouse wheel.
fn reveal(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, slot: Slot) {
    if pane.view.revealed == pane.ask.selected_result {
        return;
    }
    let clip = ui.clip_rect();
    let gap = ui.spacing().item_spacing.y + SLACK;
    let rect = slot.rect;
    let wants_down = rect.bottom() > clip.bottom() && rect.top() > clip.top() + gap;
    let wants_up = rect.top() < clip.top() && rect.bottom() < clip.bottom() - gap;
    if slot.is_drawn && !wants_down && !wants_up {
        pane.view.revealed = pane.ask.selected_result;
    } else {
        ui.scroll_to_rect(rect, None);
    }
}

fn card(
    ui: &mut egui::Ui,
    pane: &mut Pane<'_, '_>,
    item: &ResultItem,
    row: &Row,
    number_width: f32,
) {
    let is_selected = pane.is_selected(item.number);
    let card = Card::new()
        .selected(is_selected)
        .clickable(&row.open_label)
        .copy(copy_label(item.kind))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = space::SM;
            let chip_clicked = head(ui, item, row, is_selected, number_width);
            let preview_clicked = preview(ui, pane, item, row);
            chip_clicked || preview_clicked
        });
    if card.response.clicked() || card.inner {
        pane.select(item.number);
    }
    if card.copied {
        pane.cx.intents.push(Intent::CopyText(item.text.clone()));
    }
}

fn head(
    ui: &mut egui::Ui,
    item: &ResultItem,
    row: &Row,
    is_selected: bool,
    number_width: f32,
) -> bool {
    let kind = Kind::from(item.kind);
    let mut chip_clicked = false;
    ui.horizontal(|ui| {
        let at_right = egui::Layout::right_to_left(egui::Align::Center);
        let chip = CitationChip::new(item.number).selected(is_selected);
        chip_clicked = ui
            .allocate_ui_with_layout(egui::vec2(number_width, size::CHIP), at_right, |ui| {
                ui.add(chip)
            })
            .inner
            .clicked();
        ui.add(
            Badge::new(&row.kind_label)
                .tone(kind.tone())
                .icon(kind.icon()),
        );
        let place_width = (ui.available_width() - SCORE_WIDTH - space::SM).max(0.0);
        ui.allocate_ui_with_layout(
            egui::vec2(place_width, size::CHIP),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add(egui::Label::new(TextRole::Small.rich(ui, &row.place)).truncate());
            },
        );
        ui.with_layout(at_right, |ui| {
            let score = TextRole::Label
                .rich(ui, &row.score)
                .color(theme::palette(ui.ctx()).text_secondary);
            ui.label(score).on_hover_ui(|ui| {
                ui.label(TextRole::Small.rich(ui, SCORE_HELP));
            });
        });
    });
    ui.label(TextRole::Small.rich(ui, &row.reason));
    chip_clicked
}

/// Returns true when a picture or a formula in the preview was clicked.
fn preview(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, item: &ResultItem, row: &Row) -> bool {
    let salt = ("preview", item.number);
    match item.kind {
        ItemKind::Chunk => {
            scrolled(ui, salt, TEXT_PREVIEW, |ui| {
                pane.rich(ui, &body(&item.text))
            });
            false
        }
        ItemKind::Formula => {
            if let Some(name) = &item.name {
                ui.label(TextRole::Small.rich(ui, name));
            }
            let formula = math::MathRef::block(&item.text);
            math::show(ui, pane.cx.media, &formula).clicked()
        }
        ItemKind::Figure => figure(ui, pane, item, row),
        ItemKind::Table => {
            if let Some(caption) = &item.caption {
                pane.rich(ui, &body(caption));
            }
            scrolled(ui, salt, TABLE_PREVIEW, |ui| {
                pane.table(ui, &item.text, TextRole::Body);
            });
            false
        }
        ItemKind::Code => {
            if let Some(caption) = &item.caption {
                pane.rich(ui, &body(caption));
            }
            scrolled(ui, salt, CODE_PREVIEW, |ui| {
                pane.code(ui, &item.text, item.language.as_deref());
            });
            false
        }
    }
}

/// The picture of a figure beside its words, or over them in a narrow pane, where words beside
/// the picture would have room for only a few letters.
fn figure(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, item: &ResultItem, row: &Row) -> bool {
    if pane.is_narrow {
        let clicked = thumbnail(ui, pane, item, row);
        figure_words(ui, pane, item);
        return clicked;
    }
    let mut clicked = false;
    ui.horizontal_top(|ui| {
        clicked = thumbnail(ui, pane, item, row);
        ui.vertical(|ui| figure_words(ui, pane, item));
    });
    clicked
}

/// True when the picture was clicked.
fn thumbnail(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, item: &ResultItem, row: &Row) -> bool {
    let Some(image) = &item.image else {
        return false;
    };
    let mut clicked = false;
    let top_down = egui::Layout::top_down(egui::Align::Min);
    ui.allocate_ui_with_layout(THUMBNAIL, top_down, |ui| {
        ui.set_min_size(THUMBNAIL);
        let media = &mut *pane.cx.media;
        let response = images::show(ui, media, image, &row.kind_label, THUMBNAIL);
        crate::widgets::interactive_cursor(ui, &response);
        clicked = response.clicked();
    });
    clicked
}

fn figure_words(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, item: &ResultItem) {
    scrolled(ui, ("preview", item.number), TEXT_PREVIEW, |ui| {
        if let Some(caption) = &item.caption {
            pane.rich(ui, &body(caption));
        }
        pane.rich(ui, &body(&item.text));
    });
}

fn body(text: &str) -> rich_text::RichText<'_> {
    rich_text::RichText::new(text, TextRole::Body)
}

/// Shows the contents in a box of at most `max_height`. What is longer scrolls inside the box,
/// and when the box is at its end the list under it takes the wheel.
fn scrolled(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash + std::fmt::Debug,
    max_height: f32,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    egui::ScrollArea::vertical()
        .id_salt(salt)
        .max_height(max_height)
        .auto_shrink([false, true])
        .animated(false)
        .show(ui, add_contents);
}
