//! A page that has no picture, shown as the text of its pieces in reading order.

use eframe::egui;

use super::room_for_bar;
use crate::panels::source::tabs::{SourceTab, TabCx};
use crate::panels::source::{piece, row};
use crate::theme::{TextRole, Tone, color, radius, space};

const NOTE: &str = "This document has no page pictures. This is its text.";

pub(super) fn show(ui: &mut egui::Ui, cx: &mut TabCx<'_>) {
    let (area, bar) = room_for_bar(ui.available_rect_before_wrap());
    row::show_at(ui, bar, |ui| {
        ui.label(TextRole::Small.rich(NOTE).color(color::TEXT_MUTED));
    });
    if piece::is_scrolled_by_person(ui) {
        *cx.reveal = None;
    }
    let list = egui::UiBuilder::new().max_rect(area);
    ui.scope_builder(list, |ui| list_of_pieces(ui, cx));
}

fn list_of_pieces(ui: &mut egui::Ui, cx: &mut TabCx<'_>) {
    let page = cx.page;
    egui::ScrollArea::vertical()
        .id_salt(cx.scroll_id(SourceTab::Page))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for piece in &page.pieces {
                let is_marked = cx
                    .target_piece
                    .is_some_and(|target| target.number == piece.number);
                let fill = if is_marked {
                    Tone::Blue.swatch().wash
                } else {
                    egui::Color32::TRANSPARENT
                };
                let framed = egui::Frame::NONE
                    .inner_margin(space::XS)
                    .corner_radius(radius::SM)
                    .fill(fill)
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        piece::body(ui, piece, cx);
                    });
                if *cx.reveal == Some(piece.number) {
                    piece::bring_into_view(&framed.response);
                }
            }
        });
}
