//! Draws the whole window: the background, the top bar, and the panels of the open tab, each in
//! the rectangle the layout gives it.

use eframe::egui;

use super::layout::{self, ShellRects};
use super::top_bar;
use crate::contract::{self, Intent};
use crate::media::Media;
use crate::panels::{
    Locals, PanelCx, answer, ask_bar, concept_graph, follow_up, ingest, library, notices,
    retrieval_path, source,
};
use crate::state::Shared;
use crate::theme::color;
use crate::widgets;

/// Gives `draw` a child ui whose area is exactly `rect`, clipped to it.
pub(super) fn region(
    ui: &mut egui::Ui,
    name: &str,
    rect: egui::Rect,
    draw: impl FnOnce(&mut egui::Ui),
) {
    let builder = egui::UiBuilder::new()
        .id_salt(name)
        .max_rect(rect)
        .layout(egui::Layout::top_down(egui::Align::Min));
    ui.scope_builder(builder, |ui| {
        ui.set_clip_rect(rect);
        draw(ui);
    });
}

pub(super) fn draw(
    ui: &mut egui::Ui,
    shared: &Shared,
    locals: &mut Locals,
    media: &mut Media,
    intents: &mut Vec<Intent>,
) {
    let window = ui.max_rect();
    // The test renderer ignores the clear colour, so the window paints its own background.
    ui.painter().rect_filled(window, 0.0, color::CANVAS);
    let rects = layout::shell(window);
    let mut cx = PanelCx {
        shared,
        media,
        intents,
    };
    egui::Panel::top("top_bar")
        .exact_size(layout::TOP_BAR)
        .show_separator_line(false)
        .frame(egui::Frame::NONE)
        .show(ui, |ui| draw_top_bar(ui, &rects, locals, &mut cx));
    egui::CentralPanel::no_frame().show(ui, |ui| {
        ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());
        draw_tab(ui, &rects, locals, &mut cx);
    });
}

fn draw_top_bar(ui: &mut egui::Ui, rects: &ShellRects, locals: &mut Locals, cx: &mut PanelCx<'_>) {
    let places = layout::top_bar(rects.top_bar);
    region(ui, "logo", places.logo, |ui| {
        widgets::logo(ui);
    });
    region(ui, "tabs", places.tabs, |ui| top_bar::tabs(ui, cx));
    region(ui, "notices", places.cluster, |ui| {
        notices::show(ui, &mut locals.notices, cx);
    });
}

fn draw_tab(ui: &mut egui::Ui, rects: &ShellRects, locals: &mut Locals, cx: &mut PanelCx<'_>) {
    match cx.shared.tab {
        contract::Tab::Ask => {
            region(ui, "ask_bar", rects.ask_bar, |ui| {
                ask_bar::show(ui, &mut locals.ask_bar, cx);
            });
            region(ui, "answer", rects.answer, |ui| {
                answer::show(ui, &mut locals.answer, cx);
            });
            region(ui, "source", rects.source, |ui| {
                source::show(ui, &mut locals.source, cx);
            });
            region(ui, "concept_graph", rects.concept_graph, |ui| {
                concept_graph::show(ui, &mut locals.concept_graph, cx);
            });
            region(ui, "retrieval_path", rects.retrieval_path, |ui| {
                retrieval_path::show(ui, &mut locals.retrieval_path, cx);
            });
            region(ui, "follow_up", rects.follow_up, |ui| {
                follow_up::show(ui, &mut locals.follow_up, cx);
            });
        }
        contract::Tab::Library => region(ui, "library", rects.page, |ui| {
            library::show(ui, &mut locals.library, cx);
        }),
        contract::Tab::Ingest => region(ui, "ingest", rects.page, |ui| {
            ingest::show(ui, &mut locals.ingest, cx);
        }),
    }
}
