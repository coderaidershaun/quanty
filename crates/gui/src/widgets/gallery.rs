//! One screen that shows the colours, the text styles and every widget in every state, so a
//! reviewer sees the whole kit at once.

use eframe::egui;

use crate::theme::TextRole;

#[derive(Debug, Default)]
pub struct GalleryState {
    /// The name of the widget that was last used.
    pub last_activated: Option<&'static str>,
}

pub fn show(ui: &mut egui::Ui, state: &mut GalleryState) {
    ui.label(TextRole::Title.rich("Gallery"));
    ui.label(TextRole::Body.rich(format!(
        "Last used: {}",
        state.last_activated.unwrap_or("nothing yet")
    )));
}
