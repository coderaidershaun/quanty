//! The spinner and the progress bar. The spinner is the only one the app may use: the stock
//! egui spinner asks for a repaint on every frame, so an idle window would never rest.

use std::f32::consts::TAU;
use std::time::Duration;

use eframe::egui::{self, Color32, Rect, Response, WidgetInfo, WidgetType};

use crate::theme::{Tone, color, motion, size};

const DOTS: usize = 8;
const DOT_RADIUS: f32 = 1.5;
const BAR_HEIGHT: f32 = 4.0;

/// A ring of dots with one bright dot that moves ten times a second. `label` is its name.
pub fn spinner(ui: &mut egui::Ui, label: &str) -> Response {
    let side = egui::Vec2::splat(size::ICON_MD);
    let (rect, response) = ui.allocate_exact_size(side, egui::Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::ProgressIndicator, true, label));
    paint_spinner(ui, rect, color::TEXT);
    response
}

/// Draws the ring in `rect`, and asks for the next step. A widget that shows a spinner inside
/// itself calls this. The ask is for a repaint after a delay, never at once.
pub(super) fn paint_spinner(ui: &egui::Ui, rect: Rect, bright: Color32) {
    if !ui.is_rect_visible(rect) {
        return;
    }
    let step = ui.input(|input| (input.time / f64::from(motion::SPINNER_STEP)) as usize);
    let head = step % DOTS;
    let ring = rect.width().min(rect.height()) / 2.0 - DOT_RADIUS - 0.5;
    for dot in 0..DOTS {
        let angle = TAU * dot as f32 / DOTS as f32;
        let centre = rect.center() + ring * egui::vec2(angle.cos(), angle.sin());
        let age = (dot + DOTS - head) % DOTS;
        let fade = 1.0 - age as f32 / DOTS as f32;
        ui.painter()
            .circle_filled(centre, DOT_RADIUS, bright.gamma_multiply(fade));
    }
    // egui takes the frame time off a delayed repaint, so it is added back here. The delay then
    // stays near a tenth of a second on any screen, and never turns into an immediate repaint.
    let frame = ui.input(|input| Duration::try_from_secs_f32(input.predicted_dt));
    ui.ctx().request_repaint_after(
        Duration::from_secs_f32(motion::SPINNER_STEP) + frame.unwrap_or_default(),
    );
}

/// A bar filled to `fraction`, from 0.0 to 1.0. `label` is its name.
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

#[cfg(test)]
mod tests {
    use egui_kittest::kittest::Queryable;

    use super::*;
    use crate::state::Shared;
    use crate::testkit;

    #[test]
    fn a_visible_spinner_lets_the_harness_settle() {
        let mut harness = testkit::panel([200.0, 100.0], Shared::default(), |ui, _cx| {
            spinner(ui, "Loading the answer");
            progress_bar(ui, "Reading the page", 0.4);
        });
        // `run` panics when a widget asks for a repaint on every frame.
        harness.run();
        harness.get_by_role_and_label(
            egui::accesskit::Role::ProgressIndicator,
            "Loading the answer",
        );
        harness.get_by_role_and_label(egui::accesskit::Role::ProgressIndicator, "Reading the page");
    }
}
