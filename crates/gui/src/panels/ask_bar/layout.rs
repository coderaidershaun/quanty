//! Where each control of the bar sits. Every control has a fixed slot, so nothing moves when a
//! media title is long or the window is narrow.

use eframe::egui;

use crate::theme::{size, space, stroke};
use crate::widgets;

pub(super) const CONTENT_HEIGHT: f32 = size::CONTROL_LG + space::SM + size::CONTROL_SM;
/// SMELL: this repeats the least width of a large button, which the kit keeps to itself. If the
/// kit makes its buttons wider, Ask and Stop leave their slot.
const ASK_WIDTH: f32 = 2.4 * size::CONTROL_LG;
const PICKER_WIDTH: f32 = 4.5 * size::CONTROL_LG;
/// The category list holds short words, so its slot is narrower and leaves the note more room.
const CATEGORY_WIDTH: f32 = 3.5 * size::CONTROL_LG;

#[derive(Debug, Clone, Copy)]
pub(super) struct Rects {
    pub(super) question: egui::Rect,
    pub(super) mode: egui::Rect,
    pub(super) ask: egui::Rect,
    pub(super) media: egui::Rect,
    pub(super) authors: egui::Rect,
    pub(super) tags: egui::Rect,
    pub(super) category: egui::Rect,
    pub(super) note: egui::Rect,
}

/// `content` is exactly `CONTENT_HEIGHT` high.
pub(super) fn rects(content: egui::Rect) -> Rects {
    let first =
        egui::Rect::from_min_size(content.min, egui::vec2(content.width(), size::CONTROL_LG));
    let second = egui::Rect::from_min_size(
        egui::pos2(content.left(), first.bottom() + space::SM),
        egui::vec2(content.width(), size::CONTROL_SM),
    );
    let ask = egui::Rect::from_min_max(
        egui::pos2(first.right() - ASK_WIDTH, first.top()),
        first.max,
    );
    let mode = egui::Rect::from_min_max(
        egui::pos2(ask.left() - space::SM - PICKER_WIDTH, first.top()),
        egui::pos2(ask.left() - space::SM, first.bottom()),
    );
    let question = egui::Rect::from_min_max(
        first.min,
        egui::pos2(mode.left() - space::SM, first.bottom()),
    );
    let picker = |position: f32| {
        egui::Rect::from_min_size(
            egui::pos2(
                second.left() + position * (PICKER_WIDTH + space::SM),
                second.top(),
            ),
            egui::vec2(PICKER_WIDTH, size::CONTROL_SM),
        )
    };
    let tags = picker(2.0);
    let category = egui::Rect::from_min_size(
        egui::pos2(tags.right() + space::SM, second.top()),
        egui::vec2(CATEGORY_WIDTH, size::CONTROL_SM),
    );
    let note = egui::Rect::from_min_max(
        egui::pos2(category.right() + space::MD, second.top()),
        second.max,
    );
    Rects {
        question,
        mode,
        ask,
        media: picker(0.0),
        authors: picker(1.0),
        tags,
        category,
        note,
    }
}

/// The border is taken off the vertical margin, so the card is exactly `CONTENT_HEIGHT` plus
/// twice `space::MD` high, which is the height the window gives the bar.
pub(super) fn card() -> egui::Frame {
    widgets::panel_frame().inner_margin(egui::Margin::symmetric(
        space::LG as i8,
        (space::MD - stroke::BORDER) as i8,
    ))
}

/// The slot is not clipped, because the kit paints focus rings and glows outside a control.
pub(super) fn slot<R>(
    ui: &mut egui::Ui,
    name: &str,
    rect: egui::Rect,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let builder = egui::UiBuilder::new()
        .id_salt(name)
        .max_rect(rect)
        .layout(egui::Layout::left_to_right(egui::Align::Center));
    ui.scope_builder(builder, add).inner
}
