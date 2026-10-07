//! The top bar: the logo, and a strip of tabs once more than one tab is built. It also says
//! which parts of the app are built, so a shortcut for a part that is not built stays unbound.

use eframe::egui;

use super::layout;
// SMELL: the shell draws this bar, and this bar uses the shell's `region` to place its parts,
// so the two files need each other.
use super::shell::region;
use crate::contract::{self, Intent, Shortcut};
use crate::panels::PanelCx;
use crate::theme::{Icon, Tone};
use crate::widgets::{self, TabStrip};

/// The tabs that have a screen. The strip is drawn only when there is more than one.
pub(super) const BUILT_TABS: [contract::Tab; 1] = [contract::Tab::Ask];

/// False for a shortcut that leads to a part that is not built: a tab that is not in
/// `BUILT_TABS`, the help sheet and the health check.
pub(super) fn is_bound(shortcut: &Shortcut) -> bool {
    match shortcut.intent {
        Intent::OpenTab(tab) => BUILT_TABS.contains(&tab),
        Intent::ToggleHelp | Intent::RecheckHealth => false,
        _ => true,
    }
}

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

pub(super) fn draw(ui: &mut egui::Ui, bar: egui::Rect, cx: &mut PanelCx<'_>) {
    let places = layout::top_bar(bar);
    region(ui, "logo", places.logo, |ui| {
        widgets::logo(ui);
    });
    if BUILT_TABS.len() > 1 {
        region(ui, "tabs", places.tabs, |ui| tabs(ui, cx));
    }
}

fn tabs(ui: &mut egui::Ui, cx: &mut PanelCx<'_>) {
    let tabs: Vec<widgets::Tab<'_>> = BUILT_TABS
        .iter()
        .map(|tab| widgets::Tab::new(label(*tab)).icon(icon(*tab)))
        .collect();
    let active = BUILT_TABS
        .iter()
        .position(|tab| *tab == cx.shared.tab)
        .unwrap_or_default();
    if let Some(index) = TabStrip::new(&tabs, active).tone(Tone::Magenta).show(ui) {
        cx.intents.push(Intent::OpenTab(BUILT_TABS[index]));
    }
}
