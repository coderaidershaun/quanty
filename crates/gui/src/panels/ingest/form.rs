//! The fields of the form: the file, the book, the author and the tags.

use eframe::egui;

use super::Local;
use super::books::{self, BookChoice, Offer};
use crate::contract::{Catalogue, Intent, Loadable};
use crate::panels::PanelCx;
use crate::theme::{TextRole, color, space};
use crate::widgets::{Button, ControlSize, Dropdown, TextInput};

/// The button that goes back to the list of books is this wide.
const BACK_WIDTH: f32 = 200.0;

/// The last row of the list of books.
const ADD_A_NEW_BOOK: &str = "Add a new book…";

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

/// The book: a list of the books of the library with `Add a new book…` last, or a box for the
/// title of a new book. With no book in the library the box shows at once, with the reason.
fn book_row(ui: &mut egui::Ui, local: &mut Local, cx: &PanelCx<'_>) {
    caption(ui, "Book");
    let catalogue = &cx.shared.library.catalogue;
    let offers = catalogue.ready().map(books::offers).unwrap_or_default();
    let is_new = matches!(local.book, BookChoice::New(_));
    if offers.is_empty() {
        ui.label(TextRole::Small.rich(why_no_list(catalogue)));
        new_book(ui, local, false);
    } else if is_new {
        new_book(ui, local, true);
    } else {
        list(ui, local, &offers);
    }
}

/// Why the library offers no book to choose from.
fn why_no_list(catalogue: &Loadable<Catalogue>) -> &str {
    match catalogue {
        Loadable::Idle | Loadable::Loading => "The library is still loading.",
        Loadable::Failed(failure) => &failure.hint,
        Loadable::Ready(catalogue) if catalogue.documents().next().is_none() => {
            "No book is stored yet."
        }
        Loadable::Ready(_) => {
            "No stored document names its book, so there is no list to choose from. Type the book's title."
        }
    }
}

fn list(ui: &mut egui::Ui, local: &mut Local, offers: &[Offer<'_>]) {
    let mut rows: Vec<&str> = offers.iter().map(|offer| offer.title).collect();
    rows.push(ADD_A_NEW_BOOK);
    let chosen = match &local.book {
        BookChoice::Existing(title) => offers.iter().position(|offer| offer.title == title),
        _ => None,
    };
    let picked = Dropdown::new("ingest_book_list", "Book", &rows)
        .selected(chosen)
        .placeholder("Choose a book")
        .size(ControlSize::Medium)
        .width(ui.available_width())
        .show(ui);
    match picked.map(|index| offers.get(index)) {
        Some(Some(offer)) => local.choose(offer),
        Some(None) => local.start_new_book(),
        None => {}
    }
}

/// The box of the title of a new book, with a button back to the list when there is a list.
fn new_book(ui: &mut egui::Ui, local: &mut Local, has_list: bool) {
    let mut text = match &local.book {
        BookChoice::Unchosen => String::new(),
        BookChoice::Existing(title) | BookChoice::New(title) => title.clone(),
    };
    ui.horizontal(|ui| {
        let back = if has_list {
            BACK_WIDTH + ui.spacing().item_spacing.x
        } else {
            0.0
        };
        let typed = TextInput::new("ingest_book", "Book title", &mut text)
            .placeholder("The new book's title")
            .width((ui.available_width() - back).max(0.0))
            .show(ui);
        if typed.response.changed() {
            local.book = BookChoice::New(text);
        }
        if has_list
            && ui
                .add(Button::secondary("Choose from the library").min_width(BACK_WIDTH))
                .clicked()
        {
            local.book = BookChoice::Unchosen;
        }
    });
}
