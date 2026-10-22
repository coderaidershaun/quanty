//! The name of the app, set as a wordmark.

use eframe::egui::{self, FontId, Response};

use crate::theme::{TextRole, color};

const WORDMARK_SIZE: f32 = 20.0;

pub fn logo(ui: &mut egui::Ui) -> Response {
    let font = FontId {
        size: WORDMARK_SIZE,
        ..TextRole::BodyStrong.font()
    };
    ui.label(
        egui::RichText::new("Quanty")
            .font(font)
            .color(color::TEXT_HEADING),
    )
}
