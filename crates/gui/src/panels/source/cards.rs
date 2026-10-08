//! The Figures, Formulas and Tables tabs: one card for each piece of that kind on the page.

use eframe::egui;

use super::piece;
use super::states::whole_area;
use super::tabs::{CardTab, SourceTab, TabCx};
use crate::theme::{Icon, Kind};
use crate::widgets::{Card, Placeholder};

pub(super) fn show(ui: &mut egui::Ui, tab: CardTab, cx: &mut TabCx<'_>) {
    let (page, index) = (cx.page, cx.index);
    let places = index.of(tab);
    if places.is_empty() {
        let (icon, title) = nothing_of(tab);
        whole_area(ui, Placeholder::empty(icon, title));
        return;
    }
    if piece::is_scrolled_by_person(ui) {
        *cx.reveal = None;
    }
    egui::ScrollArea::vertical()
        .id_salt(cx.scroll_id(SourceTab::Cards(tab)))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for piece in places.iter().filter_map(|place| page.pieces.get(*place)) {
                let is_marked = cx
                    .target_piece
                    .is_some_and(|target| target.number == piece.number);
                let tag = piece
                    .label
                    .as_deref()
                    .unwrap_or_else(|| piece::kind_name(piece.kind));
                let shown = ui
                    .push_id(piece.number, |ui| {
                        Card::new()
                            .tag(Kind::from(piece.kind), tag)
                            .selected(is_marked)
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                piece::body(ui, piece, cx);
                            })
                    })
                    .inner;
                if *cx.reveal == Some(piece.number) {
                    piece::bring_into_view(&shown.response);
                }
            }
        });
}

fn nothing_of(tab: CardTab) -> (Icon, &'static str) {
    match tab {
        CardTab::Figures => (Icon::FIGURE, "No figures on this page"),
        CardTab::Formulas => (Icon::FORMULA, "No formulas on this page"),
        CardTab::Tables => (Icon::TABLE, "No tables on this page"),
    }
}
