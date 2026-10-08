//! The second row of the bar: the book, author and tags filters, and what each one offers.

use std::iter::once;

use eframe::egui;

use super::layout::{Rects, slot};
use crate::contract::Filters;
use crate::state::Library;
use crate::theme::{Icon, color, size};
use crate::widgets::Dropdown;

const ALL_BOOKS: &str = "All books";
const ALL_AUTHORS: &str = "All authors";
const ALL_TAGS: &str = "All tags";

#[derive(Debug, Default)]
pub(super) struct Choices {
    /// The library revision these lists were built from. `None` means they are not built yet.
    revision: Option<u64>,
    /// True when the lists come from a catalogue that has loaded.
    is_known: bool,
    /// "All books" comes first, then each titled book that has a chapter in catalogue order. A
    /// book with no chapter would be a filter that finds nothing.
    books: Vec<String>,
    /// "All authors" comes first, then each author once, sorted.
    authors: Vec<String>,
    /// The list holds each tag once, sorted.
    tags: Vec<String>,
}

impl Choices {
    pub(super) fn refresh(&mut self, library: &Library) -> bool {
        if self.revision == Some(library.revision) {
            return false;
        }
        self.revision = Some(library.revision);
        let catalogue = library.catalogue.ready();
        self.is_known = catalogue.is_some();
        self.books = once(ALL_BOOKS)
            .chain(
                catalogue
                    .into_iter()
                    .flat_map(|catalogue| &catalogue.books)
                    .filter(|book| !book.chapters.is_empty())
                    .filter_map(|book| book.title.as_deref()),
            )
            .map(str::to_owned)
            .collect();
        self.authors = once(ALL_AUTHORS)
            .chain(
                catalogue
                    .into_iter()
                    .flat_map(|catalogue| catalogue.authors()),
            )
            .map(str::to_owned)
            .collect();
        self.tags = catalogue
            .into_iter()
            .flat_map(|catalogue| catalogue.tags())
            .map(str::to_owned)
            .collect();
        true
    }

    /// A filter on a book that is gone would find nothing and not say why.
    pub(super) fn keep_known(&self, filters: &mut Filters) {
        if !self.is_known {
            return;
        }
        // Row 0 of a list is the "All" row, which is no value.
        let is_offered =
            |options: &[String], value: &str| options.iter().skip(1).any(|option| option == value);
        filters.book.take_if(|book| !is_offered(&self.books, book));
        filters
            .author
            .take_if(|author| !is_offered(&self.authors, author));
        filters.tags.retain(|tag| self.tags.contains(tag));
    }

    pub(super) fn show(&self, ui: &mut egui::Ui, rects: &Rects, filters: &mut Filters) {
        slot(ui, "books", rects.books, |ui| {
            let pick = Pick {
                label: "Books",
                all: ALL_BOOKS,
                options: &self.books,
                width: rects.books.width(),
            };
            pick_one(ui, &pick, &mut filters.book);
        });
        slot(ui, "authors", rects.authors, |ui| {
            let pick = Pick {
                label: "Authors",
                all: ALL_AUTHORS,
                options: &self.authors,
                width: rects.authors.width(),
            };
            pick_one(ui, &pick, &mut filters.author);
        });
        slot(ui, "tags", rects.tags, |ui| {
            pick_several(ui, &self.tags, rects.tags.width(), &mut filters.tags);
        });
    }
}

/// Row 0 is "All …" and means no filter.
struct Pick<'a> {
    label: &'a str,
    all: &'a str,
    options: &'a [String],
    width: f32,
}

fn pick_one(ui: &mut egui::Ui, pick: &Pick<'_>, value: &mut Option<String>) {
    let selected = match value.as_deref() {
        None => Some(0),
        Some(current) => pick.options.iter().position(|option| option == current),
    };
    let has_choice = pick.options.len() > 1;
    ui.add_enabled_ui(has_choice, |ui| {
        // A value that is not in the list, because the library has not loaded, shows as the
        // placeholder.
        let shown = value.as_deref().unwrap_or(pick.all);
        let chosen = Dropdown::new(pick.label, pick.options)
            .selected(selected)
            .placeholder(shown)
            .width(pick.width)
            .show(ui);
        if let Some(index) = chosen {
            *value = (index > 0).then(|| pick.options[index].clone());
        }
    });
}

/// The kit has no list of which the person ticks any number of rows.
fn pick_several(ui: &mut egui::Ui, options: &[String], width: f32, chosen: &mut Vec<String>) {
    let summary = match chosen.as_slice() {
        [] => ALL_TAGS.to_owned(),
        [only] => only.clone(),
        many => format!("{} tags", many.len()),
    };
    ui.add_enabled_ui(!options.is_empty(), |ui| {
        let combo = egui::ComboBox::from_id_salt("tags")
            .selected_text(summary)
            .width(width)
            .truncate()
            .icon(paint_caret)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show_ui(ui, |ui| {
                if ui.selectable_label(chosen.is_empty(), ALL_TAGS).clicked() {
                    chosen.clear();
                }
                for tag in options {
                    let mut is_chosen = chosen.contains(tag);
                    if ui.checkbox(&mut is_chosen, tag.as_str()).changed() {
                        if is_chosen {
                            chosen.push(tag.clone());
                            chosen.sort();
                        } else {
                            chosen.retain(|kept| kept != tag);
                        }
                    }
                }
            });
        ui.ctx()
            .accesskit_node_builder(combo.response.id, |node| node.set_label("Tags"));
    });
}

fn paint_caret(ui: &egui::Ui, rect: egui::Rect, _: &egui::style::WidgetVisuals, _: bool) {
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        Icon::CARET_DOWN.glyph(),
        Icon::font(size::ICON_SM),
        color::TEXT_SECONDARY,
    );
}
