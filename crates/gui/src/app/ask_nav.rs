//! The controls that choose which view of the Ask tab shows: the strip of tabs over the side
//! panel of a medium window, and the bar along the foot of a compact one.

use eframe::egui;

use crate::panels::PanelCx;
use crate::theme::Icon;
use crate::widgets::{NavBar, NavItem, Tab, TabStrip};
use contract::{AskView, Intent};

fn label(view: AskView) -> &'static str {
    match view {
        AskView::Answer => "Answer",
        AskView::Source => "Source",
        AskView::ConceptGraph => "Graph",
        AskView::RetrievalPath => "Path",
        AskView::FollowUp => "Follow up",
    }
}

fn icon(view: AskView) -> Icon {
    match view {
        AskView::Answer => Icon::ANSWER,
        AskView::Source => Icon::BOOK,
        AskView::ConceptGraph => Icon::GRAPH,
        AskView::RetrievalPath => Icon::PATH,
        AskView::FollowUp => Icon::FOLLOW_UP,
    }
}

pub(super) fn side_tabs(ui: &mut egui::Ui, cx: &mut PanelCx<'_>) {
    // The strip draws its line on the bottom edge of its room, where the clip would cut half of it
    // off, so the clip is grown by one point.
    ui.set_clip_rect(ui.clip_rect().expand2(egui::vec2(0.0, 1.0)));
    let tabs: Vec<Tab<'_>> = AskView::SIDE
        .iter()
        .map(|view| Tab::new(label(*view)))
        .collect();
    let active = AskView::SIDE
        .iter()
        .position(|view| *view == cx.shared.side_view())
        .unwrap_or_default();
    let chosen = TabStrip::new(&tabs, active)
        .show(ui)
        .and_then(|index| AskView::SIDE.get(index));
    if let Some(view) = chosen {
        cx.intents.push(Intent::ShowAskView(*view));
    }
}

pub(super) fn bottom_nav(ui: &mut egui::Ui, cx: &mut PanelCx<'_>) {
    let items = AskView::ALL.map(|view| NavItem {
        icon: icon(view),
        label: label(view),
    });
    let active = AskView::ALL
        .iter()
        .position(|view| *view == cx.shared.ask_view)
        .unwrap_or_default();
    let chosen = NavBar::new(&items, active)
        .show(ui)
        .and_then(|index| AskView::ALL.get(index));
    if let Some(view) = chosen {
        cx.intents.push(Intent::ShowAskView(*view));
    }
}
