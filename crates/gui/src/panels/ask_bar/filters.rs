//! The media, document, author, tags and category filters, and what each one offers. They stand
//! on the second row of the bar, and the ones that have no room there stand in the sheet that
//! the Filters button opens.

use std::iter::once;

use eframe::egui;

use super::layout::{FILTER_COUNT, Rects, slot};
use crate::state::Library;
use crate::widgets::{ControlSize, Dropdown, paint_caret, style_list_rows};
use contract::{Category, DocId, Document, Filters};

const ALL_MEDIA: &str = "All media";
const ALL_DOCUMENTS: &str = "All documents";
const ONE_DOCUMENT: &str = "One document";
const ALL_AUTHORS: &str = "All authors";
const ALL_TAGS: &str = "All tags";
const ANY_CATEGORY: &str = "Any category";

/// What the Document list offers for one row of the media list.
#[derive(Debug)]
struct DocumentList {
    /// "All documents" first, then each document of the media as the source picker names it, in
    /// the catalogue's order, which is the reading order.
    labels: Vec<String>,
    /// The id of each document, in the order of `labels` after its first row.
    ids: Vec<DocId>,
}

impl DocumentList {
    fn of(documents: &[Document]) -> DocumentList {
        DocumentList {
            labels: once(ALL_DOCUMENTS.to_owned())
                .chain(documents.iter().map(|document| document.name().label()))
                .collect(),
            ids: documents.iter().map(|document| document.id).collect(),
        }
    }
}

/// What the Document list offers before the lists are built.
static NO_DOCUMENTS: DocumentList = DocumentList {
    labels: Vec::new(),
    ids: Vec::new(),
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Filter {
    Media,
    Document,
    Authors,
    Tags,
    Category,
}

impl Filter {
    /// In the order the filters stand on the bar.
    pub(super) const ALL: [Filter; FILTER_COUNT] = [
        Filter::Media,
        Filter::Document,
        Filter::Authors,
        Filter::Tags,
        Filter::Category,
    ];

    fn salt(self) -> &'static str {
        match self {
            Filter::Media => "media",
            Filter::Document => "document",
            Filter::Authors => "authors",
            Filter::Tags => "tags",
            Filter::Category => "category",
        }
    }

    /// Whether the draft narrows the ask by this filter. Tags count once, however many.
    pub(super) fn is_set(self, filters: &Filters) -> bool {
        match self {
            Filter::Media => filters.media.is_some(),
            Filter::Document => filters.document.is_some(),
            Filter::Authors => filters.author.is_some(),
            Filter::Tags => !filters.tags.is_empty(),
            Filter::Category => filters.category.is_some(),
        }
    }
}

/// The room a filter has: how wide its box is, and how tall.
#[derive(Debug, Clone, Copy)]
pub(super) struct Room {
    pub(super) width: f32,
    pub(super) size: ControlSize,
}

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
    /// One list for each row of `media`. The row of "All media" holds only "All documents".
    documents: Vec<DocumentList>,
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
        self.media = vec![ALL_MEDIA.to_owned()];
        self.documents = vec![DocumentList::of(&[])];
        let with_documents = catalogue
            .into_iter()
            .flat_map(|catalogue| &catalogue.media)
            .filter(|media| !media.documents.is_empty());
        for media in with_documents {
            if let Some(title) = media.title.as_deref() {
                self.media.push(title.to_owned());
                self.documents.push(DocumentList::of(&media.documents));
            }
        }
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

    /// The documents to offer while `media` is the chosen media, or none is.
    fn documents_of(&self, media: Option<&str>) -> &DocumentList {
        let row = media
            .and_then(|media| self.media.iter().skip(1).position(|title| title == media))
            .map_or(0, |place| place + 1);
        self.documents.get(row).unwrap_or(&NO_DOCUMENTS)
    }

    /// A filter on a media, a document, an author or a tag that is gone would find nothing and
    /// not say why. Every category is always offered, so the category is kept.
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
        let offered = &self.documents_of(filters.media.as_deref()).ids;
        filters
            .document
            .take_if(|document| !offered.contains(document));
    }

    /// Draws the filters that have a slot on the bar.
    pub(super) fn show(&self, ui: &mut egui::Ui, rects: &Rects, filters: &mut Filters) {
        for (filter, slot_rect) in Filter::ALL.into_iter().zip(rects.filters) {
            let Some(rect) = slot_rect else {
                continue;
            };
            let room = Room {
                width: rect.width(),
                size: ControlSize::Small,
            };
            slot(ui, filter.salt(), rect, |ui| {
                self.show_one(ui, filter, room, filters);
            });
        }
    }

    /// Draws one filter in the `ui` it is given: its slot on the bar, or a line of the sheet.
    pub(super) fn show_one(
        &self,
        ui: &mut egui::Ui,
        filter: Filter,
        room: Room,
        filters: &mut Filters,
    ) {
        match filter {
            Filter::Media => {
                let pick = Pick {
                    label: "Media",
                    all: ALL_MEDIA,
                    options: &self.media,
                    room,
                };
                if pick_one(ui, &pick, &mut filters.media) {
                    self.keep_known(filters);
                }
            }
            Filter::Document => {
                let list = self.documents_of(filters.media.as_deref());
                pick_document(ui, room, list, &mut filters.document);
            }
            Filter::Authors => {
                let pick = Pick {
                    label: "Authors",
                    all: ALL_AUTHORS,
                    options: &self.authors,
                    room,
                };
                pick_one(ui, &pick, &mut filters.author);
            }
            Filter::Tags => pick_several(ui, &self.tags, room, &mut filters.tags),
            Filter::Category => {
                ui.add_enabled_ui(self.has_documents, |ui| {
                    pick_category(ui, room, &mut filters.category);
                });
            }
        }
    }
}

/// Row 0 is "All …" and means no filter.
struct Pick<'a> {
    label: &'a str,
    all: &'a str,
    options: &'a [String],
    room: Room,
}

/// Whether the person chose a row that differs from the value before.
fn pick_one(ui: &mut egui::Ui, pick: &Pick<'_>, value: &mut Option<String>) -> bool {
    let selected = match value.as_deref() {
        None => Some(0),
        Some(current) => pick.options.iter().position(|option| option == current),
    };
    let has_choice = pick.options.len() > 1;
    ui.add_enabled_ui(has_choice, |ui| {
        // A value that is not in the list, because the library has not loaded, shows as the
        // placeholder.
        let shown = value.as_deref().unwrap_or(pick.all);
        let chosen = Dropdown::new(pick.label, pick.options, pick.room.width)
            .selected(selected)
            .placeholder(shown)
            .size(pick.room.size)
            .show(ui);
        let Some(index) = chosen else {
            return false;
        };
        let picked = (index > 0).then(|| pick.options[index].clone());
        let is_changed = picked != *value;
        *value = picked;
        is_changed
    })
    .inner
}

/// Row 0 is "All documents" and means no filter. The list is off until a media gives it more
/// rows.
fn pick_document(ui: &mut egui::Ui, room: Room, list: &DocumentList, value: &mut Option<DocId>) {
    let selected = place_of(&list.ids, *value);
    // A document that is not in the list, because the library did not load again, must not read
    // as "All documents" while the ask still asks that one document.
    let placeholder = if value.is_some() {
        ONE_DOCUMENT
    } else {
        ALL_DOCUMENTS
    };
    ui.add_enabled_ui(list.labels.len() > 1, |ui| {
        // The Source panel has a list named "Document", so this one has its own name.
        let chosen = Dropdown::new("Document filter", &list.labels, room.width)
            .selected(selected)
            .placeholder(placeholder)
            .size(room.size)
            .show(ui);
        if let Some(index) = chosen {
            *value = value_at(&list.ids, index);
        }
    });
}

/// Row 0 is "Any category" and means no filter. The rows after it follow `Category::ALL`.
fn pick_category(ui: &mut egui::Ui, room: Room, value: &mut Option<Category>) {
    let [book, paper, other] = Category::ALL.map(Category::label);
    let options = [ANY_CATEGORY, book, paper, other];
    let selected = place_of(&Category::ALL, *value);
    let chosen = Dropdown::new("Category", &options, room.width)
        .selected(selected)
        .size(room.size)
        .show(ui);
    if let Some(index) = chosen {
        *value = value_at(&Category::ALL, index);
    }
}

/// The row of `value` in a list whose row 0 means no filter and whose next rows follow `values`.
fn place_of<T: Copy + PartialEq>(values: &[T], value: Option<T>) -> Option<usize> {
    match value {
        None => Some(0),
        Some(current) => values
            .iter()
            .position(|&each| each == current)
            .map(|place| place + 1),
    }
}

/// The value on a chosen row of a list whose row 0 means no filter.
fn value_at<T: Copy>(values: &[T], index: usize) -> Option<T> {
    values.get(index.checked_sub(1)?).copied()
}

/// The kit has no list of which the person ticks any number of rows.
fn pick_several(ui: &mut egui::Ui, options: &[String], room: Room, chosen: &mut Vec<String>) {
    let summary = match chosen.as_slice() {
        [] => ALL_TAGS.to_owned(),
        [only] => only.clone(),
        many => format!("{} tags", many.len()),
    };
    ui.add_enabled_ui(!options.is_empty(), |ui| {
        ui.spacing_mut().interact_size.y = room.size.height();
        let combo = egui::ComboBox::from_id_salt("tags")
            .selected_text(summary)
            .width(room.width)
            .truncate()
            .icon(paint_caret)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show_ui(ui, |ui| {
                // Only this row loses its edge: an unchecked check box below would vanish without it.
                let all = ui.scope(|ui| {
                    style_list_rows(ui, room.size);
                    ui.selectable_label(chosen.is_empty(), ALL_TAGS)
                });
                if all.inner.clicked() {
                    chosen.clear();
                }
                for tag in options {
                    let mut is_chosen = chosen.contains(tag);
                    let response = ui.checkbox(&mut is_chosen, tag.as_str());
                    crate::widgets::interactive_cursor(ui, &response);
                    if response.changed() {
                        if is_chosen {
                            chosen.push(tag.clone());
                            chosen.sort();
                        } else {
                            chosen.retain(|kept| kept != tag);
                        }
                    }
                }
            });
        crate::widgets::interactive_cursor(ui, &combo.response);
        ui.ctx()
            .accesskit_node_builder(combo.response.id, |node| node.set_label("Tags"));
    });
}
