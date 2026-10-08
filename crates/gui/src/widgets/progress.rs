//! The spinner and the progress bars. These are the only ones the app may use: the stock egui
//! spinner asks for a repaint on every frame, so an idle window would never rest.

use std::f32::consts::TAU;
use std::time::Duration;

use eframe::egui::{self, Color32, Rect, Response, WidgetInfo, WidgetType};

use crate::theme::{Tone, color, motion, size};

const DOTS: usize = 8;
const DOT_RADIUS: f32 = 1.5;
const RING_INSET: f32 = 0.5;
const BAR_HEIGHT: f32 = 4.0;
/// Seconds that the lit part of a bar with no count takes to cross it once.
const SLIDE: f64 = 1.5;
/// The share of the track that the lit part of a bar with no count covers.
const LIT_SHARE: f32 = 1.0 / 3.0;

pub fn spinner(ui: &mut egui::Ui, label: &str) -> Response {
    let side = egui::Vec2::splat(size::ICON_MD);
    let (rect, response) = ui.allocate_exact_size(side, egui::Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::ProgressIndicator, true, label));
    paint_spinner(ui, rect, color::TEXT);
    response
}

pub(super) fn paint_spinner(ui: &egui::Ui, rect: Rect, bright: Color32) {
    if !ui.is_rect_visible(rect) {
        return;
    }
    let step = ui.input(|input| (input.time / f64::from(motion::SPINNER_STEP)) as usize);
    let head = step % DOTS;
    let ring = rect.width().min(rect.height()) / 2.0 - DOT_RADIUS - RING_INSET;
    for dot in 0..DOTS {
        let angle = TAU * dot as f32 / DOTS as f32;
        let centre = rect.center() + ring * egui::vec2(angle.cos(), angle.sin());
        let age = (head + DOTS - dot) % DOTS;
        let fade = 1.0 - age as f32 / DOTS as f32;
        ui.painter()
            .circle_filled(centre, DOT_RADIUS, bright.gamma_multiply(fade));
    }
    ask_for_the_next_step(ui);
}

fn ask_for_the_next_step(ui: &egui::Ui) {
    // egui takes the frame time off a delayed repaint, so it is added back here. The delay then
    // stays near a tenth of a second on any screen, and never turns into an immediate repaint.
    let frame = ui.input(|input| Duration::try_from_secs_f32(input.predicted_dt));
    ui.ctx().request_repaint_after(
        Duration::from_secs_f32(motion::SPINNER_STEP) + frame.unwrap_or_default(),
    );
}

pub fn progress_bar(ui: &mut egui::Ui, label: &str, fraction: f32) -> Response {
    let fraction = fraction.clamp(0.0, 1.0);
    let side = egui::vec2(ui.available_width(), BAR_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(side, egui::Sense::hover());
    response.widget_info(|| WidgetInfo {
        value: Some(f64::from(fraction)),
        ..WidgetInfo::labeled(WidgetType::ProgressIndicator, true, label)
    });
    if ui.is_rect_visible(rect) {
        let rounding = BAR_HEIGHT / 2.0;
        ui.painter().rect_filled(rect, rounding, color::BORDER);
        let mut filled = rect;
        filled.set_width(rect.width() * fraction);
        ui.painter()
            .rect_filled(filled, rounding, Tone::Blue.swatch().solid);
    }
    response
}

/// A bar for work whose length is not known yet: a lit part slides across the track, and the bar
/// asks for its next frame as the spinner does.
pub fn indeterminate_bar(ui: &mut egui::Ui, label: &str) -> Response {
    let side = egui::vec2(ui.available_width(), BAR_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(side, egui::Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::ProgressIndicator, true, label));
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let rounding = BAR_HEIGHT / 2.0;
    ui.painter().rect_filled(rect, rounding, color::BORDER);
    let phase = ui.input(|input| (input.time / SLIDE).fract()) as f32;
    let width = rect.width();
    let left = rect.left() + (phase * (1.0 + LIT_SHARE) - LIT_SHARE) * width;
    let lit = Rect::from_x_y_ranges(
        left.max(rect.left())..=(left + LIT_SHARE * width).min(rect.right()),
        rect.y_range(),
    );
    ui.painter()
        .rect_filled(lit, rounding, Tone::Blue.swatch().solid);
    ask_for_the_next_step(ui);
    response
}
