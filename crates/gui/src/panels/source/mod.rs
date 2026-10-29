//! The page of the chapter that an answer stands on, or that the reader of the Library shows,
//! with its pieces and concepts.

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

use crate::panels::{PanelCx, PanelWidth};
use crate::state::SourceTarget;
use crate::theme::space;
use crate::widgets::{Surface, panel_frame};
use contract::Loadable;
use follow::marked_piece;
use tabs::{SourceTab, TabCx};

pub(super) use piece::{body as piece_body, copy_label, kind_name};

/// Below this inner width the panel is narrow, so it stacks its header and makes its main controls
/// tall enough for a finger. The panel of the smallest wide window is 386 points inside and a
/// phone's is at most 341, so a wide window is never narrow.
const NARROW_BELOW: f32 = 380.0;

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

/// Where the panel is drawn. The Ask tab keeps it beside the answer, with a button that gives it
/// the whole tab, and the Library keeps it beside a document, with a button that closes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Host {
    Ask,
    Library,
}

pub fn show(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>) {
    draw(ui, local, cx, Host::Ask);
}

/// The panel as the Library draws it, in the room the caller gives it.
pub fn show_reader(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>) {
    draw(ui, local, cx, Host::Library);
}

fn draw(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>, host: Host) {
    let shared = cx.shared;
    local.follow(shared);
    panel_frame(Surface::Sheet).show(ui, |ui| {
        ui.set_min_size(ui.available_size());
        ui.spacing_mut().item_spacing = egui::vec2(space::SM, space::SM);
        let width = PanelWidth::of(ui, NARROW_BELOW);
        header::show(ui, &mut local.header, cx, width, host);
        let is_anything_open = !matches!(shared.source.page, Loadable::Idle);
        let list = tabs::tab_list(&shared.source, &local.index);
        let has_failed_concepts = matches!(shared.source.concepts, Loadable::Failed(_));
        let concepts_warning = has_failed_concepts.then_some(concepts::FAILED_TITLE);
        let chosen = ui
            .add_enabled_ui(is_anything_open, |ui| {
                tabs::strip(ui, &list, local.tab, concepts_warning, width)
            })
            .inner;
        if let Some(tab) = chosen {
            local.tab = tab;
            local.reveal = None;
        }
        body(ui, local, cx, width);
        if local.reveal.is_some() && cx.media.is_idle() {
            local.reveal = None;
        }
    });
}

fn body(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>, width: PanelWidth) {
    let nav = &cx.shared.source;
    let page = nav.page.ready();
    if let Some(page) = page {
        page::prefetch(&mut *cx.media, page);
    }
    match (local.tab, page) {
        (SourceTab::Concepts, _) => concepts::show(ui, cx, width),
        (_, None) => states::show(ui, cx, width),
        (tab, Some(page)) => {
            let mut tab_cx = TabCx {
                page,
                target_piece: marked_piece(nav, page),
                index: &local.index,
                reveal: &mut local.reveal,
                generation: nav.generation,
                width,
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
