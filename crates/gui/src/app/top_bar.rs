//! What the tab strip of the top bar shows, and what a click on a tab asks for.

use eframe::egui;

use crate::contract::{self, Intent};
use crate::panels::PanelCx;
use crate::theme::{Icon, Tone};
use crate::widgets::{self, TabStrip};

fn label(tab: contract::Tab) -> &'static str {
    match tab {
        contract::Tab::Ask => "Ask",
        contract::Tab::Library => "Library",
        contract::Tab::Ingest => "Ingest",
    }
}

fn icon(tab: contract::Tab) -> Icon {
    match tab {
        contract::Tab::Ask => Icon::SEARCH,
        contract::Tab::Library => Icon::LIBRARY,
        contract::Tab::Ingest => Icon::INGEST,
    }
}

pub(super) fn tabs(ui: &mut egui::Ui, cx: &mut PanelCx<'_>) {
    let tabs: Vec<widgets::Tab<'_>> = contract::Tab::ALL
        .iter()
        .map(|tab| widgets::Tab::new(label(*tab)).icon(icon(*tab)))
        .collect();
    let active = contract::Tab::ALL
        .iter()
        .position(|tab| *tab == cx.shared.tab)
        .unwrap_or_default();
    if let Some(index) = TabStrip::new(&tabs, active).tone(Tone::Magenta).show(ui) {
        cx.intents.push(Intent::OpenTab(contract::Tab::ALL[index]));
    }
}
