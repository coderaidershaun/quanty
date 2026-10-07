//! The fields of the form: the file, the book, the author and the tags.

use eframe::egui;

use super::Local;
use crate::contract::Intent;
use crate::panels::PanelCx;
use crate::theme::{TextRole, color, space};
use crate::widgets::{Button, ControlSize, Dropdown, TextInput};

/// The books of the library are offered in a list this wide beside the box of the title.
const BOOKS_WIDTH: f32 = 240.0;

pub(super) const RULE: &str =
    "The file must be named chapter-<number>-<name>.pdf, for example chapter-3-greeks.pdf.";
pub(super) const COST: &str = "Checking is free. Starting is paid work: claude and Jev convert each page, Gemini embeds the items, and claude reads the concepts.";

/// Draws the form. The whole of it is faded out while an ingest runs.
pub(super) fn show(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>) {
    let is_running = cx.shared.ingest.is_running();
    ui.label(TextRole::Heading.rich("Add a chapter"));
    ui.add_space(space::MD);
    ui.add_enabled_ui(!is_running, |ui| {
        file_row(ui, local, cx.intents);
        ui.label(TextRole::Small.rich(RULE));
        ui.add_space(space::MD);
        book_row(ui, local, cx);
        caption(ui, "Author");
        TextInput::new("ingest_author", "Author", &mut local.author)
            .placeholder("Optional")
            .show(ui);
        caption(ui, "Tags");
        TextInput::new("ingest_tags", "Tags", &mut local.tags)
            .placeholder("Optional, with commas between them")
            .show(ui);
    });
    ui.add_space(space::SM);
    ui.label(TextRole::Small.rich(COST));
    ui.add_space(space::LG);
}

fn caption(ui: &mut egui::Ui, text: &str) {
    ui.add_space(space::SM);
    ui.label(TextRole::Label.rich(text));
}

fn file_row(ui: &mut egui::Ui, local: &Local, intents: &mut Vec<Intent>) {
    ui.horizontal(|ui| {
        if ui.add(Button::secondary("Choose a PDF")).clicked() {
            intents.push(Intent::PickPdf);
        }
        let name = local
            .pdf
            .as_deref()
            .and_then(|pdf| pdf.file_name())
            .map(|name| name.to_string_lossy().into_owned());
        let shown = match name {
            Some(name) => TextRole::Body.rich(name),
            None => TextRole::Body
                .rich("No file chosen")
                .color(color::TEXT_MUTED),
        };
        ui.add(egui::Label::new(shown).truncate());
    });
}

/// The box of the title, with the books of the library beside it when the library has loaded.
fn book_row(ui: &mut egui::Ui, local: &mut Local, cx: &PanelCx<'_>) {
    caption(ui, "Book title");
    let titles: Vec<&str> = cx
        .shared
        .library
        .catalogue
        .ready()
        .into_iter()
        .flat_map(|catalogue| &catalogue.books)
        .filter_map(|book| book.title.as_deref())
        .collect();
    let list_width = if titles.is_empty() {
        0.0
    } else {
        BOOKS_WIDTH + ui.spacing().item_spacing.x
    };
    ui.horizontal(|ui| {
        let width = (ui.available_width() - list_width).max(0.0);
        TextInput::new("ingest_book", "Book title", &mut local.book)
            .placeholder("The book this chapter is from")
            .width(width)
            .show(ui);
        if titles.is_empty() {
            return;
        }
        let chosen = titles.iter().position(|title| *title == local.book);
        let picked = Dropdown::new("ingest_books", "Books in the library", &titles)
            .selected(chosen)
            .placeholder("Use a book of the library")
            .size(ControlSize::Medium)
            .width(BOOKS_WIDTH)
            .show(ui);
        if let Some(index) = picked {
            local.book = titles[index].to_owned();
        }
    });
}
