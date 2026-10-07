//! The mark and the name of the app.

use eframe::egui::{self, Response};

use crate::theme::TextRole;

/// The word "quanty" in the title style.
pub fn logo(ui: &mut egui::Ui) -> Response {
    ui.label(TextRole::Title.rich("quanty"))
}
