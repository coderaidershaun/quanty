//! The footer: what is known of each paid account, and the state of the connection. A wide window
//! shows all of it on one line. A smaller one shows a slim row, and a tap on it opens the full
//! lines above the row.

use eframe::egui::{self, Rect, pos2, vec2};

use super::balances;
use super::layout::{Form, ShellRects};
use super::scheme_switch;
use crate::panels::PanelCx;
use crate::theme::{self, TextRole, space};
use crate::widgets;
use contract::Connection;

/// The room at the right end of a wide footer that is kept for the connection.
const CONNECTION_WIDTH: f32 = 240.0;
/// The most width that the full lines take when the slim row opens them.
const DETAILS_WIDTH: f32 = 440.0;

pub(super) fn draw(
    ui: &mut egui::Ui,
    rects: &ShellRects,
    connection: Connection,
    cx: &mut PanelCx<'_>,
) {
    let footer = rects.footer;
    let palette = theme::palette(ui.ctx());
    let painter = ui.painter();
    let line = theme::hairline(painter, palette.hairline);
    painter.hline(footer.x_range(), footer.top() + line.width / 2.0, line);
    let row = footer.shrink2(vec2(rects.form.gutter(), 0.0));
    let text = connection_text(connection);
    match rects.form {
        Form::Wide => wide(ui, row, &text, cx),
        Form::Medium | Form::Compact => slim(ui, row, &text, rects.form, cx),
    }
}

fn wide(ui: &mut egui::Ui, row: Rect, text: &str, cx: &mut PanelCx<'_>) {
    let connection = Rect::from_min_max(pos2(row.right() - CONNECTION_WIDTH, row.top()), row.max);
    let accounts = Rect::from_min_max(row.min, pos2(connection.left() - space::LG, row.bottom()));
    balances::show(ui, accounts, cx);
    ui.put(connection, egui::Label::new(TextRole::Small.rich(ui, text)));
}

fn slim(ui: &mut egui::Ui, row: Rect, text: &str, form: Form, cx: &mut PanelCx<'_>) {
    let builder = egui::UiBuilder::new()
        .id_salt("footer row")
        .max_rect(row)
        .layout(egui::Layout::left_to_right(egui::Align::Center));
    let is_tapped = ui
        .scope_builder(builder, |ui| {
            let is_tapped = balances::slim(ui, cx);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(egui::Label::new(TextRole::Small.rich(ui, text)).truncate());
            });
            is_tapped
        })
        .inner;
    egui::Popup::new(
        egui::Id::new("footer details"),
        ui.ctx().clone(),
        egui::PopupAnchor::ParentRect(row),
        ui.layer_id(),
    )
    .open_memory(is_tapped.then_some(egui::SetOpenCommand::Toggle))
    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
    .align(egui::RectAlign::TOP_START)
    .width(row.width().min(DETAILS_WIDTH))
    .show(|ui| {
        balances::lines(ui, cx);
        widgets::separator(ui);
        ui.label(TextRole::Small.rich(ui, text));
        // A medium window has the switch in its top bar. A compact one has no room there.
        if form == Form::Compact {
            widgets::separator(ui);
            scheme_switch::row(ui, cx);
        }
    });
}

fn connection_text(connection: Connection) -> String {
    match connection {
        Connection::Connected => "Connected".to_owned(),
        Connection::Reconnecting { attempt } => format!("Reconnecting · attempt {attempt}"),
        Connection::Lost => "Connection lost".to_owned(),
    }
}
