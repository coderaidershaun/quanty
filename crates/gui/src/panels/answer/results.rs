//! The list of results, one card for each, with a preview cut at a fixed height.

use eframe::egui;

use super::heights::Slot;
use super::listing::Row;
use super::pane::Pane;
use super::phase::AnswerTab;
use crate::contract::{ItemKind, ResultItem};
use crate::media::{images, math, rich_text};
use crate::theme::{Icon, Kind, TextRole, color, size, space};
use crate::widgets::{Badge, Card, CitationChip, Placeholder};

/// Three lines of the small text.
const TEXT_PREVIEW: f32 = 3.0 * TextRole::Small.line_height();
const THUMBNAIL: egui::Vec2 = egui::vec2(3.0 * TEXT_PREVIEW, 2.0 * TEXT_PREVIEW);
/// A table's header and two rows.
const TABLE_PREVIEW: f32 = 2.0 * TEXT_PREVIEW;
const SCORE_WIDTH: f32 = size::CONTROL_LG;
/// Rounding to whole pixels can move a row by a hair. This is the room that allows for it.
const SLACK: f32 = 1.0;
const SCORE_HELP: &str = "How close the item is to the question. Higher is closer.";

/// The results this tab lists, in the order of the search and under their own numbers.
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
        AnswerTab::Answer | AnswerTab::Results => None,
    }
}

fn rows(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, tab: AnswerTab) {
    let (ask, Some(listing)) = (pane.ask, pane.listing) else {
        return;
    };
    let mut heights = std::mem::take(&mut pane.view.row_heights);
    for (place, (item, row)) in ask.results().iter().zip(&listing.rows).enumerate() {
        if !tab.lists(item.kind) {
            continue;
        }
        let slot = heights.show(ui, place, |ui| card(ui, pane, item, row));
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

fn card(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, item: &ResultItem, row: &Row) {
    let is_selected = pane.is_selected(item.number);
    let card = Card::new()
        .selected(is_selected)
        .clickable(&row.open_label)
        .show(ui, |ui| {
            let chip_clicked = head(ui, item, row, is_selected);
            let preview_clicked = preview(ui, pane, item, row);
            chip_clicked || preview_clicked
        });
    if card.response.clicked() || card.inner {
        pane.select(item.number);
    }
}

/// The chip, the kind, the place and the score on one line, and the reason under it. Returns
/// true when the chip was clicked.
fn head(ui: &mut egui::Ui, item: &ResultItem, row: &Row, is_selected: bool) -> bool {
    let kind = Kind::from(item.kind);
    let mut chip_clicked = false;
    ui.horizontal(|ui| {
        chip_clicked = ui
            .add(CitationChip::new(item.number).selected(is_selected))
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
                ui.add(egui::Label::new(TextRole::Small.rich(&row.place)).truncate());
            },
        );
        let right = egui::Layout::right_to_left(egui::Align::Center);
        ui.with_layout(right, |ui| {
            let score = TextRole::Label
                .rich(&row.score)
                .color(color::TEXT_SECONDARY);
            ui.label(score).on_hover_ui(|ui| {
                ui.label(TextRole::Small.rich(SCORE_HELP));
            });
        });
    });
    ui.label(TextRole::Small.rich(&row.reason));
    chip_clicked
}

/// The start of the item, cut at a fixed height. Returns true when a picture or a formula in
/// it was clicked.
fn preview(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, item: &ResultItem, row: &Row) -> bool {
    match item.kind {
        ItemKind::Chunk => {
            clipped(ui, TEXT_PREVIEW, |ui| pane.rich(ui, &small(&item.text)));
            false
        }
        ItemKind::Formula => {
            if let Some(name) = &item.name {
                ui.label(TextRole::Small.rich(name));
            }
            let formula = math::MathRef::block(&item.text);
            math::show(ui, pane.cx.media, &formula).clicked()
        }
        ItemKind::Figure => {
            let mut clicked = false;
            ui.horizontal_top(|ui| {
                if let Some(image) = &item.image {
                    let top_down = egui::Layout::top_down(egui::Align::Min);
                    ui.allocate_ui_with_layout(THUMBNAIL, top_down, |ui| {
                        ui.set_min_size(THUMBNAIL);
                        let media = &mut *pane.cx.media;
                        clicked =
                            images::show(ui, media, image, &row.kind_label, THUMBNAIL).clicked();
                    });
                }
                ui.vertical(|ui| {
                    clipped(ui, THUMBNAIL.y, |ui| {
                        if let Some(caption) = &item.caption {
                            pane.rich(ui, &small(caption));
                        }
                        pane.rich(ui, &small(&item.text));
                    });
                });
            });
            clicked
        }
        ItemKind::Table => {
            if let Some(caption) = &item.caption {
                pane.rich(ui, &small(caption));
            }
            clipped(ui, TABLE_PREVIEW, |ui| {
                pane.table(ui, &item.text, TextRole::Small);
            });
            false
        }
    }
}

fn small(text: &str) -> rich_text::RichText<'_> {
    rich_text::RichText::new(text, TextRole::Small)
}

/// Draws `add_contents` in a box at most `max_height` high, cuts what does not fit, and takes
/// only the room it used. A cut has no ellipsis.
fn clipped(ui: &mut egui::Ui, max_height: f32, add_contents: impl FnOnce(&mut egui::Ui)) {
    let top_left = ui.cursor().min;
    let width = ui.available_width();
    let window = egui::Rect::from_min_size(top_left, egui::vec2(width, max_height));
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt("preview").max_rect(window));
    child.set_clip_rect(window.intersect(ui.clip_rect()));
    add_contents(&mut child);
    let used = child.min_rect().height().min(max_height);
    ui.advance_cursor_after_rect(egui::Rect::from_min_size(top_left, egui::vec2(width, used)));
}
