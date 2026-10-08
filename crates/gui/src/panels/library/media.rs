//! One media of the library: its title, its labels, and its documents.

use eframe::egui;

use super::{Local, document};
use crate::contract::Media;
use crate::panels::PanelCx;
use crate::panels::labels::category_and_authors;
use crate::theme::{TextRole, color, space};
use crate::widgets::Badge;

const NO_MEDIA: &str = "No media";
const NO_DOCUMENT: &str = "No document yet. Add one on the Ingest tab.";

pub(super) fn show(ui: &mut egui::Ui, media: &Media, local: &mut Local, cx: &mut PanelCx<'_>) {
    ui.label(TextRole::BodyStrong.rich(media.title.as_deref().unwrap_or(NO_MEDIA)));
    // The documents of no media have no category to show.
    if media.title.is_some() {
        ui.horizontal_wrapped(|ui| {
            ui.label(TextRole::Small.rich(category_and_authors(media)));
            for tag in &media.tags {
                ui.add(Badge::new(tag));
            }
        });
    }
    ui.add_space(space::SM);
    if media.documents.is_empty() {
        ui.label(TextRole::Small.rich(NO_DOCUMENT).color(color::TEXT_MUTED));
    }
    for listed in &media.documents {
        document::show(ui, listed, local, cx);
        ui.add_space(space::SM);
    }
}
