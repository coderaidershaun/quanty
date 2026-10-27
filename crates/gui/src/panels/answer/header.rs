//! The row over the body: the tabs, Share at the right end with what the ask used beside it, and
//! the line under both.

use eframe::egui;

use super::pane::Pane;
use super::phase::AnswerTab;
use super::share;
use crate::panels::labels::spent_text;
use crate::theme::{self, TextRole, size, space};
use crate::widgets::{self, TabStrip};
use contract::Usage;

pub(super) fn show(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>) {
    let counts = pane.listing.map(|listing| listing.counts);
    let tabs = AnswerTab::ALL.map(|tab| {
        let tab_with_no_count = widgets::Tab::new(tab.label());
        match counts.and_then(|counts| counts.of(tab)) {
            Some(count) => tab_with_no_count.count(count),
            None => tab_with_no_count,
        }
    });
    let has_results = counts.is_some_and(|counts| counts.results > 0);
    let active = pane.active();

    let mut row = ui.available_rect_before_wrap();
    row.set_height(size::TAB);
    // Share keeps its room at every width, so tabs that do not fit scroll inside their own strip
    // and never run under Share.
    let mut strip_room = row;
    strip_room.max.x = row.right() - share::width(pane.is_narrow) - space::SM;
    // Top down, not centred: a strip built with `ui.horizontal` sits eight points low in a
    // centred row.
    let top_down = egui::Layout::top_down(egui::Align::Min);
    let strip = ui.scope_builder(
        egui::UiBuilder::new().max_rect(strip_room).layout(top_down),
        |ui| {
            ui.add_enabled_ui(has_results, |ui| {
                TabStrip::new(&tabs, active.index()).show(ui)
            })
            .inner
        },
    );
    if let Some(tab) = strip.inner.and_then(|chosen| AnswerTab::ALL.get(chosen)) {
        pane.view.tab = Some(*tab);
        pane.view.revealed = None;
    }
    let tabs_end = strip.response.rect.right();
    let right = egui::Layout::right_to_left(egui::Align::Center);
    ui.scope_builder(egui::UiBuilder::new().max_rect(row).layout(right), |ui| {
        share::show(ui, pane);
        if let Some(spent) = pane.spent() {
            spent_label(ui, spent, tabs_end);
        }
    });

    // The strip draws its own line under its tabs, but fades it while the strip is switched off.
    // The line is drawn again here, whole, so it looks the same across the row in every state.
    let painter = ui.painter();
    let line = theme::hairline(painter, theme::palette(ui.ctx()).hairline);
    painter.hline(row.left()..=row.right(), row.bottom(), line);
}

/// What the ask used, in small words left of Share. It is left out when it would reach the tabs,
/// as it does in a narrow pane, so it never covers one.
fn spent_label(ui: &mut egui::Ui, usage: &Usage, tabs_end: f32) {
    let small = TextRole::Small;
    let color = small.color(theme::palette(ui.ctx()));
    let galley = small.galley(ui, &spent_text(usage), color);
    let room = ui.available_rect_before_wrap().right() - tabs_end - space::SM;
    if galley.size().x <= room {
        ui.add(egui::Label::new(galley).selectable(false));
    }
}
