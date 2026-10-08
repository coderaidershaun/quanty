//! The page of the chapter that an answer stands on, with its pieces and concepts.

mod cards;
mod concepts;
mod follow;
mod header;
mod page;
mod piece;
mod row;
mod states;
mod tabs;

use eframe::egui;

use crate::contract::Loadable;
use crate::panels::PanelCx;
use crate::state::SourceTarget;
use crate::theme::space;
use crate::widgets::panel_frame;
use follow::marked_piece;
use tabs::{SourceTab, TabCx};

#[derive(Debug, Default)]
pub struct Local {
    /// The `generation` of the shared state that everything below belongs to.
    generation: u64,
    tab: SourceTab,
    page: page::State,
    header: header::Cache,
    /// The target, and the count of times the source was shown, that the tab and the reveal
    /// were last set for.
    followed: Option<(SourceTarget, u64)>,
    /// The number of the piece still to bring into view.
    reveal: Option<u32>,
    index: tabs::PieceIndex,
}

pub fn show(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>) {
    let shared = cx.shared;
    local.follow(shared);
    panel_frame().show(ui, |ui| {
        ui.set_min_size(ui.available_size());
        ui.spacing_mut().item_spacing = egui::vec2(space::SM, space::SM);
        header::show(ui, &mut local.header, cx);
        let is_anything_open = !matches!(shared.source.page, Loadable::Idle);
        let list = tabs::tab_list(&shared.source, &local.index);
        let chosen = ui
            .add_enabled_ui(is_anything_open, |ui| tabs::strip(ui, &list, local.tab))
            .inner;
        if let Some(tab) = chosen {
            local.tab = tab;
            local.reveal = None;
        }
        body(ui, local, cx);
        if local.reveal.is_some() && cx.media.is_idle() {
            local.reveal = None;
        }
    });
}

fn body(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>) {
    let nav = &cx.shared.source;
    let page = nav.page.ready();
    if let Some(page) = page {
        page::prefetch(&mut *cx.media, page);
    }
    match (local.tab, page) {
        (SourceTab::Concepts, _) => concepts::show(ui, cx),
        (_, None) => states::show(ui, cx),
        (tab, Some(page)) => {
            let mut tab_cx = TabCx {
                page,
                target_piece: marked_piece(nav, page),
                index: &local.index,
                reveal: &mut local.reveal,
                generation: nav.generation,
                media: &mut *cx.media,
                intents: &mut *cx.intents,
            };
            if let SourceTab::Cards(tab) = tab {
                cards::show(ui, tab, &mut tab_cx);
            } else {
                page::show(ui, &mut local.page, &mut tab_cx);
            }
        }
    }
}
