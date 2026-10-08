//! One book of the library: its title, the labels it shows, and its chapters.

use eframe::egui;

use super::{Local, chapter};
use crate::contract::Book;
use crate::panels::PanelCx;
use crate::theme::{TextRole, color, space};
use crate::widgets::Badge;

const NO_BOOK: &str = "No book";
const NO_CHAPTER: &str = "No chapter yet. Add one on the Ingest tab.";

pub(super) fn show(ui: &mut egui::Ui, book: &Book, local: &mut Local, cx: &mut PanelCx<'_>) {
    ui.label(TextRole::BodyStrong.rich(book.title.as_deref().unwrap_or(NO_BOOK)));
    let labels = book.labels();
    if labels.author.is_some() || !labels.tags.is_empty() {
        ui.horizontal_wrapped(|ui| {
            if let Some(author) = labels.author {
                ui.label(TextRole::Small.rich(author));
            }
            for tag in &labels.tags {
                ui.add(Badge::new(tag));
            }
        });
    }
    ui.add_space(space::SM);
    if book.chapters.is_empty() {
        ui.label(TextRole::Small.rich(NO_CHAPTER).color(color::TEXT_MUTED));
    }
    for document in &book.chapters {
        chapter::show(ui, document, local, cx);
        ui.add_space(space::SM);
    }
}
