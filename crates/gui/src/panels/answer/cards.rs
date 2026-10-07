//! The cards of the written answer: a formula, a figure or a table, shown as the source has it.

use eframe::egui;

use super::listing::Row;
use super::pane::{Copied, Pane};
use crate::contract::ResultItem;
use crate::media::{images, math, rich_text};
use crate::theme::{Icon, Kind, TextRole, color, size, space};
use crate::widgets::Card;

const FORMULA_TAG: &str = "Formula (verbatim from source)";
const FIGURE_TAG: &str = "Figure (from source)";
const TABLE_TAG: &str = "Table (from source)";
const COPY_LABEL: &str = "Copy LaTeX";
const COPIED_LABEL: &str = "Copied";
const OPEN_LABEL: &str = "Open in source";
/// A picture is never taller than this, so a tall figure cannot push the text after it far
/// out of sight.
const FIGURE_HEIGHT: f32 = 6.0 * size::CONTROL_LG;

/// A formula card. The formula is the result's own text, copied from the source.
pub(super) fn formula(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, item: &ResultItem, row: &Row) {
    let number = item.number;
    let is_copied = pane.view.copied == Some(Copied::Latex(number));
    let (icon, action) = if is_copied {
        (Icon::CHECK, COPIED_LABEL)
    } else {
        (Icon::COPY, COPY_LABEL)
    };
    let card = Card::new()
        .tag(Kind::Formula, FORMULA_TAG)
        .action(icon, action)
        .selected(pane.is_selected(number))
        .clickable(&row.open_label)
        .show(ui, |ui| centred_formula(ui, pane, item));
    if card.action_clicked {
        pane.copy(&item.text, Copied::Latex(number));
    }
    if card.response.clicked() || card.inner.clicked() {
        pane.select(number);
    }
    if pane.view.copied == Some(Copied::Latex(number))
        && !ui.rect_contains_pointer(card.response.rect)
    {
        pane.view.copied = None;
    }
}

pub(super) fn figure(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, item: &ResultItem, row: &Row) {
    let number = item.number;
    let card = Card::new()
        .tag(Kind::Figure, FIGURE_TAG)
        .action(Icon::OPEN, OPEN_LABEL)
        .selected(pane.is_selected(number))
        .clickable(&row.open_label)
        .show(ui, |ui| match &item.image {
            Some(image) => {
                let room = egui::vec2(ui.available_width(), FIGURE_HEIGHT);
                let picture = images::show(ui, pane.cx.media, image, &row.kind_label, room);
                caption(ui, pane, item, row);
                Some(picture)
            }
            None => {
                pane.rich(ui, &rich_text::RichText::new(&item.text, TextRole::Body));
                caption(ui, pane, item, row);
                None
            }
        });
    let picture_clicked = card.inner.is_some_and(|picture| picture.clicked());
    if card.action_clicked || card.response.clicked() || picture_clicked {
        pane.select(number);
    }
}

pub(super) fn table(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, item: &ResultItem, row: &Row) {
    let number = item.number;
    let card = Card::new()
        .tag(Kind::Table, TABLE_TAG)
        .action(Icon::OPEN, OPEN_LABEL)
        .selected(pane.is_selected(number))
        .clickable(&row.open_label)
        .show(ui, |ui| {
            caption(ui, pane, item, row);
            pane.table(ui, &item.text, TextRole::Body);
        });
    if card.action_clicked || card.response.clicked() {
        pane.select(number);
    }
}

/// The label in bold, the caption, and the chip of the result at the end of the line.
fn caption(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, item: &ResultItem, row: &Row) {
    let text = rich_text::RichText::new(&row.caption, TextRole::Body)
        .cites(std::slice::from_ref(&item.number))
        .selected(pane.ask.selected_result);
    pane.rich(ui, &text);
}

/// The formula in the middle of the card, whatever the width of its printed label, and the
/// label at the right, level with the formula.
fn centred_formula(
    ui: &mut egui::Ui,
    pane: &mut Pane<'_, '_>,
    item: &ResultItem,
) -> egui::Response {
    let label = item.label.as_deref();
    let gutter = label.map_or(0.0, |label| {
        TextRole::Label
            .galley(ui, label, color::TEXT_SECONDARY)
            .size()
            .x
            + space::MD
    });
    let outer = ui.available_rect_before_wrap();
    let room = egui::Rect::from_min_max(
        egui::pos2(outer.left() + gutter, outer.top()),
        egui::pos2(outer.right() - gutter, outer.bottom()),
    );
    let formula = math::MathRef::block(&item.text);
    let shown = ui
        .scope_builder(egui::UiBuilder::new().max_rect(room), |ui| {
            math::show(ui, pane.cx.media, &formula)
        })
        .inner;
    if let Some(label) = label {
        let at = egui::Rect::from_min_max(
            egui::pos2(outer.right() - gutter, shown.rect.top()),
            egui::pos2(outer.right(), shown.rect.bottom()),
        );
        let text = TextRole::Label.rich(label).color(color::TEXT_SECONDARY);
        ui.put(at, egui::Label::new(text));
    }
    shown
}
