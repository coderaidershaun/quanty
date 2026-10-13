//! The top bar: the logo and the strip of tabs.

use eframe::egui;

use super::layout;
use super::region::region;
use crate::contract::{self, Intent};
use crate::panels::PanelCx;
use crate::theme::{color, hairline};
use crate::widgets::{self, TabStrip};

fn label(tab: contract::Tab) -> &'static str {
    match tab {
        contract::Tab::Ask => "Ask",
        contract::Tab::Library => "Library",
        contract::Tab::Ingest => "Ingest",
    }
}

pub(super) fn draw(ui: &mut egui::Ui, bar: egui::Rect, cx: &mut PanelCx<'_>) {
    let places = layout::top_bar(bar);
    // The line runs across the whole window at the foot of the tab row, not of the bar, so the mark
    // of the open tab sits on it. It is drawn first, so that mark covers it.
    let painter = ui.painter();
    let line = hairline(painter, color::HAIRLINE);
    painter.hline(bar.x_range(), places.tabs.bottom() - line.width / 2.0, line);
    region(ui, "logo", places.logo, |ui| {
        widgets::logo(ui);
    });
    region(ui, "tabs", places.tabs, |ui| tabs(ui, cx));
}

fn tabs(ui: &mut egui::Ui, cx: &mut PanelCx<'_>) {
    let tabs: Vec<widgets::Tab<'_>> = contract::Tab::ALL
        .iter()
        .map(|tab| widgets::Tab::new(label(*tab)))
        .collect();
    let active = contract::Tab::ALL
        .iter()
        .position(|tab| *tab == cx.shared.tab)
        .unwrap_or_default();
    if let Some(index) = TabStrip::new(&tabs, active).show(ui) {
        cx.intents.push(Intent::OpenTab(contract::Tab::ALL[index]));
    }
}
