//! The fields of the form: the file, the book, the author and the tags, and the buttons that save
//! a new book without a file or give it up.

use eframe::egui;

use super::Local;
use super::books::{self, BookChoice, Offer};
use crate::contract::{Catalogue, Intent, Loadable};
use crate::panels::PanelCx;
use crate::state::BookSave;
use crate::theme::{TextRole, color, space};
use crate::widgets::{Button, ControlSize, Dropdown, Notice, TextInput};

const ADD_A_NEW_BOOK: &str = "Add a new book…";

const SAVE_NOTE: &str =
    "A saved book stays in this list with its author and tags, also before its first chapter.";

pub(super) const RULE: &str =
    "The file must be named chapter-<number>-<name>.pdf, for example chapter-3-greeks.pdf.";
pub(super) const COST: &str = "Checking is free. Starting is paid work: claude and Jev convert each page, Gemini embeds the items, and claude reads the concepts.";

pub(super) fn show(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>) {
    let is_running = cx.shared.ingest.is_running();
    ui.label(TextRole::Heading.rich("Add a chapter"));
    ui.add_space(space::MD);
    ui.add_enabled_ui(!is_running, |ui| {
        file_row(ui, local, cx.intents);
        ui.label(TextRole::Small.rich(RULE));
        ui.add_space(space::MD);
        let has_title_box = book_row(ui, local, cx);
        caption(ui, "Author");
        TextInput::new("ingest_author", "Author", &mut local.author)
            .placeholder("Optional")
            .show(ui);
        caption(ui, "Tags");
        TextInput::new("ingest_tags", "Tags", &mut local.tags)
            .placeholder("Optional, with commas between them")
            .show(ui);
        if has_title_box {
            save_row(ui, local, cx);
        }
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
        let (words, tint) = match name {
            Some(name) => (name, color::TEXT),
            None => ("No file chosen".to_owned(), color::TEXT_MUTED),
        };
        // No line height is set: a line taller than the font leaves the words above the middle
        // of the row.
        let shown = egui::RichText::new(words)
            .font(TextRole::Body.font())
            .color(tint);
        ui.add(egui::Label::new(shown).truncate());
    });
}

/// Returns whether the box for the title of a new book is on screen.
fn book_row(ui: &mut egui::Ui, local: &mut Local, cx: &PanelCx<'_>) -> bool {
    caption(ui, "Book");
    let catalogue = &cx.shared.library.catalogue;
    let offers = catalogue.ready().map(books::offers).unwrap_or_default();
    if offers.is_empty() {
        ui.label(TextRole::Small.rich(why_no_book_is_offered(catalogue)));
    }
    if let BookChoice::New(title) = &mut local.book {
        new_book(ui, title);
        true
    } else {
        list(ui, local, &offers);
        false
    }
}

fn why_no_book_is_offered(catalogue: &Loadable<Catalogue>) -> &str {
    match catalogue {
        Loadable::Idle | Loadable::Loading => "The library is still loading.",
        Loadable::Failed(failure) => &failure.hint,
        Loadable::Ready(catalogue) if catalogue.documents().next().is_none() => {
            "No book is stored yet."
        }
        Loadable::Ready(_) => "No stored document names its book. Add a new book.",
    }
}

fn list(ui: &mut egui::Ui, local: &mut Local, offers: &[Offer<'_>]) {
    let mut rows: Vec<&str> = offers.iter().map(|offer| offer.title).collect();
    rows.push(ADD_A_NEW_BOOK);
    let chosen = match &local.book {
        BookChoice::Existing(title) => offers.iter().position(|offer| offer.title == title),
        _ => None,
    };
    // A library that failed to load again must not hide the book that was chosen.
    let placeholder = match &local.book {
        BookChoice::Existing(title) if chosen.is_none() => title,
        _ => "Choose a book",
    };
    let picked = Dropdown::new("ingest_book_list", "Book", &rows)
        .selected(chosen)
        .placeholder(placeholder)
        .size(ControlSize::Medium)
        .width(ui.available_width())
        .show(ui);
    match picked.map(|index| offers.get(index)) {
        Some(Some(offer)) => local.choose(offer),
        Some(None) => local.start_new_book(),
        None => {}
    }
}

fn new_book(ui: &mut egui::Ui, title: &mut String) {
    TextInput::new("ingest_book", "Book title", title)
        .placeholder("The new book's title")
        .show(ui);
}

fn save_row(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>) {
    let save = &cx.shared.library.book_save;
    let book = local.book_to_save();
    ui.add_space(space::MD);
    ui.horizontal(|ui| {
        let button = Button::secondary("Save book").loading(save.is_saving());
        if ui.add_enabled(book.is_some(), button).clicked()
            && let Some(book) = book.clone()
        {
            cx.intents.push(Intent::SaveBook(book));
        }
        if ui
            .add_enabled(!save.is_saving(), Button::secondary("Cancel"))
            .clicked()
        {
            local.cancel_new_book();
        }
        ui.add(egui::Label::new(TextRole::Small.rich(SAVE_NOTE)).truncate());
    });
    // A person who changes the title no longer sees the refusal of the old one.
    if let BookSave::Failed {
        book: failed,
        failure,
    } = save
        && book.is_some_and(|book| book.title == failed.title)
    {
        ui.add_space(space::SM);
        Notice::error(&failure.hint).show(ui);
    }
}
