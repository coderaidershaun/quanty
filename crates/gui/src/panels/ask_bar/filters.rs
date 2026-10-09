//! The second row of the bar: the media, author, tags and category filters, and what each one
//! offers.

use std::iter::once;

use eframe::egui;

use super::layout::{Rects, slot};
use crate::contract::{Category, Filters};
use crate::state::Library;
use crate::theme::{Icon, color, size};
use crate::widgets::Dropdown;

const ALL_MEDIA: &str = "All media";
const ALL_AUTHORS: &str = "All authors";
const ALL_TAGS: &str = "All tags";
const ANY_CATEGORY: &str = "Any category";

#[derive(Debug, Default)]
pub(super) struct Choices {
    /// The library revision these lists were built from. `None` means they are not built yet.
    revision: Option<u64>,
    /// True when the lists come from a catalogue that has loaded.
    is_known: bool,
    /// True when the catalogue has loaded and holds a document, so a category can find something.
    has_documents: bool,
    /// "All media" comes first, then each titled media that has a document, in catalogue order. A
    /// media with no document would be a filter that finds nothing.
    media: Vec<String>,
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
        self.has_documents =
            catalogue.is_some_and(|catalogue| catalogue.documents().next().is_some());
        self.media = once(ALL_MEDIA)
            .chain(
                catalogue
                    .into_iter()
                    .flat_map(|catalogue| &catalogue.media)
                    .filter(|media| !media.documents.is_empty())
                    .filter_map(|media| media.title.as_deref()),
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

    /// A filter on a media, an author or a tag that is gone would find nothing and not say why.
    /// Every category is always offered, so the category is kept.
    pub(super) fn keep_known(&self, filters: &mut Filters) {
        if !self.is_known {
            return;
        }
        // Row 0 of a list is the "All" row, which is no value.
        let is_offered =
            |options: &[String], value: &str| options.iter().skip(1).any(|option| option == value);
        filters
            .media
            .take_if(|media| !is_offered(&self.media, media));
        filters
            .author
            .take_if(|author| !is_offered(&self.authors, author));
        filters.tags.retain(|tag| self.tags.contains(tag));
    }

    pub(super) fn show(&self, ui: &mut egui::Ui, rects: &Rects, filters: &mut Filters) {
        slot(ui, "media", rects.media, |ui| {
            let pick = Pick {
                label: "Media",
                all: ALL_MEDIA,
                options: &self.media,
                width: rects.media.width(),
            };
            pick_one(ui, &pick, &mut filters.media);
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
        slot(ui, "category", rects.category, |ui| {
            ui.add_enabled_ui(self.has_documents, |ui| {
                pick_category(ui, rects.category.width(), &mut filters.category);
            });
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
        let chosen = Dropdown::new(pick.label, pick.options, pick.width)
            .selected(selected)
            .placeholder(shown)
            .show(ui);
        if let Some(index) = chosen {
            *value = (index > 0).then(|| pick.options[index].clone());
        }
    });
}

/// Row 0 is "Any category" and means no filter. The rows after it follow `Category::ALL`.
fn pick_category(ui: &mut egui::Ui, width: f32, value: &mut Option<Category>) {
    let [book, paper, other] = Category::ALL.map(Category::label);
    let options = [ANY_CATEGORY, book, paper, other];
    let selected = match *value {
        None => Some(0),
        Some(current) => Category::ALL
            .iter()
            .position(|&category| category == current)
            .map(|place| place + 1),
    };
    let chosen = Dropdown::new("Category", &options, width)
        .selected(selected)
        .show(ui);
    if let Some(index) = chosen {
        *value = index
            .checked_sub(1)
            .and_then(|place| Category::ALL.get(place))
            .copied();
    }
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
