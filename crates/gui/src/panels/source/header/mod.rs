//! The top of the source panel: its title, the book and chapter pickers, and the page pager.

mod pager;
mod pickers;

use eframe::egui;

use super::row;
use crate::contract::Intent;
use crate::panels::PanelCx;
use crate::widgets::section_header;
use pager::PagerText;
use pickers::Lists;

const TITLE: &str = "Source in Context";
/// The panel's inner height from which the header has three rows. Below it the pager moves up
/// beside the title and the two pickers share a row, so the page keeps its room.
const FOLD_BELOW: f32 = 440.0;
/// The part of a row that the book picker takes when the chapter picker is beside it.
const BOOK_SHARE: f32 = 0.55;

#[derive(Debug, Default)]
pub(super) struct Cache {
    lists: Lists,
    pager: PagerText,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rows {
    Two,
    Three,
}

pub(super) fn show(ui: &mut egui::Ui, cache: &mut Cache, cx: &mut PanelCx<'_>) {
    let shared = cx.shared;
    cache.lists.refresh(shared);
    let rows = if ui.available_height() >= FOLD_BELOW {
        Rows::Three
    } else {
        Rows::Two
    };
    let [book, chapter] = cache.lists.pickers(shared);
    let (mut opened_book, mut opened_chapter, mut turn) = (None, None, None);
    match rows {
        Rows::Three => {
            section_header(ui, TITLE, |_| {});
            let room = ui.available_width();
            opened_book = book.show(ui, room);
            row::show(ui, |ui| {
                let gap = ui.spacing().item_spacing.x;
                let rest = ui.available_width() - gap - cache.pager.width();
                opened_chapter = chapter.show(ui, rest.max(0.0));
                turn = cache.pager.show(ui, &shared.source);
            });
        }
        Rows::Two => {
            turn = row::show(ui, |ui| {
                section_header(ui, TITLE, |ui| cache.pager.show(ui, &shared.source))
            });
            row::show(ui, |ui| {
                opened_book = book.show(ui, BOOK_SHARE * ui.available_width());
                opened_chapter = chapter.show(ui, ui.available_width());
            });
        }
    }
    for doc in [opened_book, opened_chapter].into_iter().flatten() {
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
