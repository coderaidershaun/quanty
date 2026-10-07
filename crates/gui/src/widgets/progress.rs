//! The spinner and the progress bar. The spinner is the only one the app may use: the stock
//! egui spinner asks for a repaint on every frame, so an idle window would never rest.

use std::time::Duration;

use eframe::egui::{self, Response, WidgetInfo, WidgetType};

use crate::theme::{Tone, color, motion, size};

const DOTS: usize = 8;

/// A ring of dots with one bright dot that moves ten times a second. `label` is its name.
pub fn spinner(ui: &mut egui::Ui, label: &str) -> Response {
    let side = egui::Vec2::splat(size::ICON_MD);
    let (rect, response) = ui.allocate_exact_size(side, egui::Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::ProgressIndicator, true, label));
    if ui.is_rect_visible(rect) {
        let step = ui.input(|input| (input.time / f64::from(motion::SPINNER_STEP)) as usize);
        let bright = step % DOTS;
        let ring = size::ICON_MD / 2.0 - 2.0;
        for dot in 0..DOTS {
            let angle = std::f32::consts::TAU * dot as f32 / DOTS as f32;
            let centre = rect.center() + ring * egui::vec2(angle.cos(), angle.sin());
            let tone = if dot == bright {
                color::TEXT
            } else {
                color::TEXT_MUTED
            };
            ui.painter().circle_filled(centre, 1.5, tone);
        }
    }
    let predicted = ui.input(|input| input.predicted_dt);
    let frame = Duration::try_from_secs_f32(predicted).unwrap_or_default();
    ui.ctx()
        .request_repaint_after(Duration::from_secs_f32(motion::SPINNER_STEP) + frame);
    response
}

/// A bar filled to `fraction`, from 0.0 to 1.0. `label` is its name.
pub fn progress_bar(ui: &mut egui::Ui, label: &str, fraction: f32) -> Response {
    let bar = egui::ProgressBar::new(fraction.clamp(0.0, 1.0)).fill(Tone::Blue.swatch().solid);
    let response = ui.add(bar);
    response.widget_info(|| WidgetInfo::labeled(WidgetType::ProgressIndicator, true, label));
    response
}
