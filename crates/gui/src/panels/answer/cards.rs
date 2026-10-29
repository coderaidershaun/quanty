//! The formulas, figures, tables and code listings of the written answer, shown as the source has
//! them. A block has no box: it is one thing to click, and the number of its result stands in its
//! caption.

use eframe::egui;

use super::blocks::narrow_to_measure;
use super::listing::{Row, copy_label};
use super::pane::Pane;
use crate::media::{images, math, rich_text};
use crate::theme::{self, TextRole, radius, size, space};
use crate::widgets::{Copyable, focus_ring};
use contract::{Intent, ResultItem};

const FORMULA_CAPTION: &str = "As printed in the source";
/// A picture is never taller than this, so a tall figure cannot push the text after it far
/// out of sight.
const FIGURE_HEIGHT: f32 = 6.0 * size::CONTROL_LG;

pub(super) fn formula(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, item: &ResultItem, row: &Row) {
    narrow_to_measure(ui);
    let number = item.number;
    let is_selected = pane.is_selected(number);
    let shown = block(ui, row, item, is_selected, |ui| {
        centred_formula(ui, pane, item);
        formula_caption(ui, pane, item);
    });
    settle(pane, item, &shown);
}

pub(super) fn figure(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, item: &ResultItem, row: &Row) {
    narrow_to_measure(ui);
    let number = item.number;
    let is_selected = pane.is_selected(number);
    let shown = block(ui, row, item, is_selected, |ui| match &item.image {
        Some(image) => {
            let room = egui::vec2(ui.available_width(), FIGURE_HEIGHT);
            let picture = images::show(ui, pane.cx.media, image, &row.kind_label, room);
            crate::widgets::interactive_cursor(ui, &picture);
            centred_caption(ui, pane, item, row);
            Some(picture)
        }
        None => {
            pane.rich(ui, &rich_text::RichText::new(&item.text, TextRole::Body));
            centred_caption(ui, pane, item, row);
            None
        }
    });
    let picture_clicked = shown
        .inner
        .as_ref()
        .is_some_and(|picture| picture.clicked());
    if picture_clicked && !shown.response.clicked() {
        pane.select(number);
    }
    settle(pane, item, &shown);
}

/// The caption stands in the reading column, a little apart from the heavy rule along the top
/// of the table, and the table runs as wide as the pane.
pub(super) fn table(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, item: &ResultItem, row: &Row) {
    let is_selected = pane.is_selected(item.number);
    let shown = block(ui, row, item, is_selected, |ui| {
        ui.spacing_mut().item_spacing.y = space::SM;
        ui.scope(|ui| {
            narrow_to_measure(ui);
            caption(ui, pane, item, row);
        });
        pane.table(ui, &item.text, TextRole::Body);
    });
    settle(pane, item, &shown);
}

/// The listing runs as wide as the pane, with its caption under it.
pub(super) fn code(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, item: &ResultItem, row: &Row) {
    let is_selected = pane.is_selected(item.number);
    let shown = block(ui, row, item, is_selected, |ui| {
        ui.spacing_mut().item_spacing.y = space::SM;
        pane.code(ui, &item.text, item.language.as_deref());
        ui.scope(|ui| {
            narrow_to_measure(ui);
            caption(ui, pane, item, row);
        });
    });
    settle(pane, item, &shown);
}

/// What `block` gives back: what the contents returned, the response of the block, and whether
/// its copy button was clicked.
struct Block<R> {
    inner: R,
    response: egui::Response,
    copied: bool,
}

/// Draws a block as one thing to click, named after its result, with a little room above and
/// below and a button that copies its text. At rest it has no fill and no outline: the pointer
/// shows that it can be clicked, and the keyboard focus shows as a ring.
fn block<R>(
    ui: &mut egui::Ui,
    row: &Row,
    item: &ResultItem,
    is_selected: bool,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> Block<R> {
    let scope = ui.scope_builder(egui::UiBuilder::new().sense(egui::Sense::click()), |ui| {
        ui.spacing_mut().item_spacing.y = space::XS;
        ui.add_space(space::SM);
        let shown = Copyable::new(copy_label(item.kind)).show(ui, add_contents);
        ui.add_space(space::SM);
        shown
    });
    scope.response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Button,
            ui.is_enabled(),
            is_selected,
            &row.open_label,
        )
    });
    if scope.response.has_focus() {
        focus_ring(ui.painter(), scope.response.rect, radius::SM);
    }
    crate::widgets::interactive_cursor(ui, &scope.response);
    Block {
        inner: scope.inner.inner,
        response: scope.response,
        copied: scope.inner.copied,
    }
}

/// A click on the block selects its result, and a click on its copy button copies the text of
/// the result.
fn settle<R>(pane: &mut Pane<'_, '_>, item: &ResultItem, shown: &Block<R>) {
    if shown.copied {
        pane.cx.intents.push(Intent::CopyText(item.text.clone()));
    }
    if shown.response.clicked() {
        pane.select(item.number);
    }
}

/// The caption of a figure, a table or a code listing, in italics, with the number of its result
/// after it.
fn caption(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, item: &ResultItem, row: &Row) {
    let text = rich_text::RichText::new(&row.caption, TextRole::Caption)
        .cites(std::slice::from_ref(&item.number))
        .selected(pane.ask.selected_result);
    pane.rich(ui, &text);
}

fn centred_caption(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, item: &ResultItem, row: &Row) {
    ui.vertical_centered(|ui| caption(ui, pane, item, row));
}

/// The formula in the middle of the column, whatever the width of its printed label such as
/// "(2.4)", and the label at the right, level with the formula. In a narrow pane a formula that
/// does not fit beside its label takes the whole column, and the label stands under it at the
/// right, as a printed book sets an equation that is too long for its line.
fn centred_formula(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, item: &ResultItem) {
    let palette = theme::palette(ui.ctx());
    let label = item.label.as_deref();
    let gutter = label.map_or(0.0, |label| {
        TextRole::Body
            .galley(ui, label, palette.text_secondary)
            .size()
            .x
            + space::MD
    });
    let outer = ui.available_rect_before_wrap();
    let formula = math::MathRef::block(&item.text);
    if pane.is_narrow
        && let Some(label) = label
        && let math::MathState::Ready(image) = pane.cx.media.math.get(&formula)
        && image.width > outer.width() - 2.0 * gutter
    {
        math::show(ui, pane.cx.media, &formula);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            ui.label(TextRole::Body.rich(ui, label).color(palette.text_secondary));
        });
        return;
    }
    let room = egui::Rect::from_min_max(
        egui::pos2(outer.left() + gutter, outer.top()),
        egui::pos2(outer.right() - gutter, outer.bottom()),
    );
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
        let text = TextRole::Body.rich(ui, label).color(palette.text_secondary);
        ui.put(at, egui::Label::new(text));
    }
}

/// The line under a formula: its caption in the middle of the column.
fn formula_caption(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, item: &ResultItem) {
    let text = rich_text::RichText::new(FORMULA_CAPTION, TextRole::Caption)
        .cites(std::slice::from_ref(&item.number))
        .selected(pane.ask.selected_result);
    ui.vertical_centered(|ui| pane.rich(ui, &text));
}
