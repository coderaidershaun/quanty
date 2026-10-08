//! The top bar: the logo and the strip of tabs. It also says which shortcuts lead to a part that
//! is not built, so that they stay unbound.

use eframe::egui;

use super::layout;
use super::region::region;
use crate::contract::{self, Intent, Shortcut};
use crate::panels::PanelCx;
use crate::theme::{Icon, Tone};
use crate::widgets::{self, TabStrip};

/// False for a shortcut that leads to a part that is not built: the help sheet and the health
/// check.
pub(super) fn is_bound(shortcut: &Shortcut) -> bool {
    !matches!(shortcut.intent, Intent::ToggleHelp | Intent::RecheckHealth)
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
    region(ui, "tabs", places.tabs, |ui| tabs(ui, cx));
}

fn tabs(ui: &mut egui::Ui, cx: &mut PanelCx<'_>) {
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
