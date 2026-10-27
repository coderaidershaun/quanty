//! The list of results, one card for each, with a preview cut at a fixed height. A text that the
//! cut takes a part of ends with `…`.

use eframe::egui;

use super::listing::Row;
use super::pane::Pane;
use super::phase::AnswerTab;
use crate::media::{code, images, math, rich_text};
use crate::panels::heights::{Heights, Slot};
use crate::theme::{self, Icon, Kind, TextRole, size, space};
use crate::widgets::{Badge, Card, CitationChip, Placeholder};
use contract::{ItemKind, ResultItem};

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
const CUT_MARK: &str = "…";

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
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = space::SM;
            let chip_clicked = head(ui, item, row, is_selected, number_width);
            let (preview_clicked, cut) = preview(ui, pane, item, row);
            (chip_clicked || preview_clicked, cut)
        });
    let (inner_clicked, cut) = card.inner;
    if card.response.clicked() || inner_clicked {
        pane.select(item.number);
    }
    if let Some(window) = cut {
        mark_cut(ui, window, card_fill(&card.response));
    }
}

// SMELL: these are the fills of a card and of the sheet under it, copied from the widget kit, so
// a change to how a card is filled must be made here too.
fn card_fill(card: &egui::Response) -> egui::Color32 {
    let palette = theme::palette(&card.ctx);
    if card.is_pointer_button_down_on() {
        palette.well_deep
    } else if card.hovered() {
        palette.well
    } else {
        palette.sheet
    }
}

/// The mark stands over a fade of the card's own fill, so a text that goes on past the box does
/// not look as if it ended there. It is drawn after the card, so nothing covers it.
fn mark_cut(ui: &mut egui::Ui, window: egui::Rect, fill: egui::Color32) {
    let mark = TextRole::Body.rich(ui, CUT_MARK);
    let width = TextRole::Body.galley(ui, CUT_MARK, fill).size().x;
    let line = egui::Rect::from_min_max(
        egui::pos2(
            window.right() - width,
            window.bottom() - TextRole::Body.line_height(),
        ),
        window.right_bottom(),
    );
    let solid_from = line.left() - space::SM;
    let fade_from = solid_from - space::LG;
    let top = line.top();
    let bottom = line.bottom();
    let mut fade = egui::Mesh::default();
    for (x, tint) in [
        (fade_from, egui::Color32::TRANSPARENT),
        (solid_from, fill),
        (window.right(), fill),
    ] {
        fade.colored_vertex(egui::pos2(x, top), tint);
        fade.colored_vertex(egui::pos2(x, bottom), tint);
    }
    for column in 0..2 {
        let at = column * 2;
        fade.add_triangle(at, at + 1, at + 2);
        fade.add_triangle(at + 1, at + 2, at + 3);
    }
    ui.painter().add(fade);
    ui.put(line, egui::Label::new(mark).selectable(false).extend());
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

/// Returns true when a picture or a formula in the preview was clicked, and the box of the text
/// when the cut took a part of it.
fn preview(
    ui: &mut egui::Ui,
    pane: &mut Pane<'_, '_>,
    item: &ResultItem,
    row: &Row,
) -> (bool, Option<egui::Rect>) {
    match item.kind {
        ItemKind::Chunk => {
            let cut = clipped(ui, TEXT_PREVIEW, |ui| pane.rich(ui, &body(&item.text)));
            (false, cut)
        }
        ItemKind::Formula => {
            if let Some(name) = &item.name {
                ui.label(TextRole::Small.rich(ui, name));
            }
            let formula = math::MathRef::block(&item.text);
            (math::show(ui, pane.cx.media, &formula).clicked(), None)
        }
        ItemKind::Figure => figure(ui, pane, item, row),
        ItemKind::Table => {
            if let Some(caption) = &item.caption {
                pane.rich(ui, &body(caption));
            }
            clipped(ui, TABLE_PREVIEW, |ui| {
                pane.table(ui, &item.text, TextRole::Body);
            });
            (false, None)
        }
        ItemKind::Code => {
            if let Some(caption) = &item.caption {
                pane.rich(ui, &body(caption));
            }
            clipped(ui, CODE_PREVIEW, |ui| {
                pane.code(ui, &item.text, item.language.as_deref());
            });
            (false, None)
        }
    }
}

/// The picture of a figure beside its words, or over them in a narrow pane, where words beside
/// the picture would have room for only a few letters.
fn figure(
    ui: &mut egui::Ui,
    pane: &mut Pane<'_, '_>,
    item: &ResultItem,
    row: &Row,
) -> (bool, Option<egui::Rect>) {
    if pane.is_narrow {
        let clicked = thumbnail(ui, pane, item, row);
        return (clicked, figure_words(ui, pane, item));
    }
    let mut clicked = false;
    let mut cut = None;
    ui.horizontal_top(|ui| {
        clicked = thumbnail(ui, pane, item, row);
        ui.vertical(|ui| {
            cut = figure_words(ui, pane, item);
        });
    });
    (clicked, cut)
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

fn figure_words(
    ui: &mut egui::Ui,
    pane: &mut Pane<'_, '_>,
    item: &ResultItem,
) -> Option<egui::Rect> {
    clipped(ui, TEXT_PREVIEW, |ui| {
        if let Some(caption) = &item.caption {
            pane.rich(ui, &body(caption));
        }
        pane.rich(ui, &body(&item.text));
    })
}

fn body(text: &str) -> rich_text::RichText<'_> {
    rich_text::RichText::new(text, TextRole::Body)
}

fn clipped(
    ui: &mut egui::Ui,
    max_height: f32,
    add_contents: impl FnOnce(&mut egui::Ui),
) -> Option<egui::Rect> {
    let top_left = ui.cursor().min;
    let width = ui.available_width();
    let window = egui::Rect::from_min_size(top_left, egui::vec2(width, max_height));
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt("preview").max_rect(window));
    child.set_clip_rect(window.intersect(ui.clip_rect()));
    add_contents(&mut child);
    let drawn = child.min_rect().height();
    let used = drawn.min(max_height);
    ui.advance_cursor_after_rect(egui::Rect::from_min_size(top_left, egui::vec2(width, used)));
    (drawn > max_height + SLACK).then_some(window)
}
