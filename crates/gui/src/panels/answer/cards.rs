//! The formulas, figures, tables and code listings of the written answer, shown as the source has
//! them. A block has no box: it is one thing to click, and the number of its result stands in its
//! caption.

use eframe::egui;

use super::blocks::narrow_to_measure;
use super::listing::Row;
use super::pane::{Copied, Pane};
use crate::media::{images, math, rich_text};
use crate::theme::{Icon, TextRole, color, radius, size, space};
use crate::widgets::{Button, CitationChip, ControlSize, focus_ring};
use contract::ResultItem;

const FORMULA_CAPTION: &str = "As printed in the source";
const COPY_LABEL: &str = "Copy LaTeX";
const COPIED_LABEL: &str = "Copied";
/// A picture is never taller than this, so a tall figure cannot push the text after it far
/// out of sight.
const FIGURE_HEIGHT: f32 = 6.0 * size::CONTROL_LG;

pub(super) fn formula(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, item: &ResultItem, row: &Row) {
    narrow_to_measure(ui);
    let number = item.number;
    let is_selected = pane.is_selected(number);
    let egui::InnerResponse {
        inner: copy_clicked,
        response,
    } = block(ui, &row.open_label, is_selected, |ui| {
        centred_formula(ui, pane, item);
        formula_caption(ui, pane, item)
    });
    if copy_clicked {
        pane.copy(&item.text, Copied::Latex(number));
    }
    if response.clicked() {
        pane.select(number);
    }
    if pane.view.copied == Some(Copied::Latex(number)) && !ui.rect_contains_pointer(response.rect) {
        pane.view.copied = None;
    }
}

pub(super) fn figure(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, item: &ResultItem, row: &Row) {
    narrow_to_measure(ui);
    let number = item.number;
    let is_selected = pane.is_selected(number);
    let egui::InnerResponse {
        inner: picture,
        response,
    } = block(ui, &row.open_label, is_selected, |ui| match &item.image {
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
    let picture_clicked = picture.is_some_and(|picture| picture.clicked());
    if response.clicked() || picture_clicked {
        pane.select(number);
    }
}

/// The caption stands in the reading column, a little apart from the heavy rule along the top
/// of the table, and the table runs as wide as the pane.
pub(super) fn table(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, item: &ResultItem, row: &Row) {
    let number = item.number;
    let is_selected = pane.is_selected(number);
    let response = block(ui, &row.open_label, is_selected, |ui| {
        ui.spacing_mut().item_spacing.y = space::SM;
        ui.scope(|ui| {
            narrow_to_measure(ui);
            caption(ui, pane, item, row);
        });
        pane.table(ui, &item.text, TextRole::Body);
    })
    .response;
    if response.clicked() {
        pane.select(number);
    }
}

/// The listing runs as wide as the pane, with its caption under it.
pub(super) fn code(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, item: &ResultItem, row: &Row) {
    let number = item.number;
    let is_selected = pane.is_selected(number);
    let response = block(ui, &row.open_label, is_selected, |ui| {
        ui.spacing_mut().item_spacing.y = space::SM;
        pane.code(ui, &item.text, item.language.as_deref());
        ui.scope(|ui| {
            narrow_to_measure(ui);
            caption(ui, pane, item, row);
        });
    })
    .response;
    if response.clicked() {
        pane.select(number);
    }
}

/// Draws a block as one thing to click, named after its result, with a little room above and
/// below. At rest it has no fill and no outline: the pointer shows that it can be clicked, and
/// the keyboard focus shows as a ring.
fn block<R>(
    ui: &mut egui::Ui,
    name: &str,
    is_selected: bool,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let scope = ui.scope_builder(egui::UiBuilder::new().sense(egui::Sense::click()), |ui| {
        ui.spacing_mut().item_spacing.y = space::XS;
        ui.add_space(space::SM);
        let inner = add_contents(ui);
        ui.add_space(space::SM);
        inner
    });
    scope.response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), is_selected, name)
    });
    if scope.response.has_focus() {
        focus_ring(ui.painter(), scope.response.rect, radius::SM);
    }
    crate::widgets::interactive_cursor(ui, &scope.response);
    let response = scope.response;
    egui::InnerResponse::new(scope.inner, response)
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
/// "(2.4)", and the label at the right, level with the formula.
fn centred_formula(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, item: &ResultItem) {
    let label = item.label.as_deref();
    let gutter = label.map_or(0.0, |label| {
        TextRole::Body
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
        let text = TextRole::Body.rich(label).color(color::TEXT_SECONDARY);
        ui.put(at, egui::Label::new(text));
    }
}

/// The line under a formula: its caption in the middle, and the button that copies the LaTeX
/// right after it. The two together are in the middle of the column. True when the button was
/// clicked.
fn formula_caption(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, item: &ResultItem) -> bool {
    let button_side = ControlSize::Small.height();
    let role = TextRole::Caption;
    // SMELL: rich text cannot measure the caption before drawing it, so this width repeats how it
    // sets the words with the numeral right after them. If that changes, the line is a little off
    // centre; the button still goes where the caption really ends.
    let words = role.galley(ui, FORMULA_CAPTION, role.color()).size().x;
    let caption_width = words + CitationChip::new(item.number).size(ui).x;

    let (_, line) = ui.allocate_space(egui::vec2(ui.available_width(), button_side));
    let left = (line.center().x - (caption_width + button_side) / 2.0).max(line.left());
    let top = line.top() + (button_side - role.line_height()) / 2.0;
    let room = egui::Rect::from_min_max(egui::pos2(left, top), line.right_bottom());
    let at_left = egui::Layout::top_down(egui::Align::Min);
    let mut caption_ui = ui.new_child(egui::UiBuilder::new().max_rect(room).layout(at_left));
    let text = rich_text::RichText::new(FORMULA_CAPTION, role)
        .cites(std::slice::from_ref(&item.number))
        .selected(pane.ask.selected_result);
    pane.rich(&mut caption_ui, &text);

    let is_copied = pane.view.copied == Some(Copied::Latex(item.number));
    let (icon, action) = if is_copied {
        (Icon::CHECK, COPIED_LABEL)
    } else {
        (Icon::COPY, COPY_LABEL)
    };
    let at = egui::Rect::from_min_size(
        egui::pos2(caption_ui.min_rect().right(), line.top()),
        egui::Vec2::splat(button_side),
    );
    ui.put(at, Button::icon_only(icon, action)).clicked()
}
