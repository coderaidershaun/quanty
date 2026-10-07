//! The row over the body: the tabs, Share at the right end, and the line under both.

use eframe::egui;

use super::pane::{Copied, Pane};
use super::phase::{AnswerTab, Phase, Written};
use crate::theme::{self, Icon, color, size};
use crate::widgets::{self, ControlSize, TabStrip};

const SHARE_WIDTH: f32 = 3.0 * size::CONTROL_MD;

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
    // Top down, not centred: a strip built with `ui.horizontal` sits eight points low in a
    // centred row.
    let top_down = egui::Layout::top_down(egui::Align::Min);
    let strip = ui.scope_builder(
        egui::UiBuilder::new().max_rect(row).layout(top_down),
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
    let right = egui::Layout::right_to_left(egui::Align::Center);
    ui.scope_builder(egui::UiBuilder::new().max_rect(row).layout(right), |ui| {
        share(ui, pane);
    });

    // The strip draws its own line under its tabs, but fades it while the strip is switched off.
    // The line is drawn again here, whole, so it looks the same across the row in every state.
    let painter = ui.painter();
    let line = theme::hairline(painter, color::HAIRLINE);
    painter.hline(row.left()..=row.right(), row.bottom(), line);
}

fn share(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>) {
    let is_copied = pane.view.copied == Some(Copied::Answer);
    let (label, icon) = if is_copied {
        ("Copied", Icon::CHECK)
    } else {
        ("Share", Icon::SHARE)
    };
    let button = widgets::Button::secondary(label)
        .icon(icon)
        .size(ControlSize::Small)
        .min_width(SHARE_WIDTH);
    let has_answer = matches!(
        pane.phase,
        Phase::Found {
            written: Written::Ready(_)
        }
    );
    let response = ui
        .add_enabled(has_answer, button)
        .on_disabled_hover_text("There is no answer to share yet.");
    if response.clicked() {
        pane.share();
    } else if is_copied && !ui.rect_contains_pointer(response.rect) {
        pane.view.copied = None;
    }
}
