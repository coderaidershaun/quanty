//! The shelf: the box that finds a media or a chapter, and the rows of the media with the
//! documents of the open one.

use eframe::egui;

use super::{Local, ShelfFit, TITLE};
use crate::panels::PanelCx;
use crate::panels::labels::NO_MEDIA;
use crate::panels::pages::media_card::category_icon;
use crate::state::Open;
use crate::theme::{Icon, TextRole, space};
use crate::widgets::{ControlSize, ListRow, Placeholder, TextInput};
use contract::{Catalogue, Category, DocId, Document, Intent, Media};

const FIND: &str = "Find";
const FIND_HINT: &str = "Find a media or chapter";
const CLEAR: &str = "Clear";
const NOTHING_MATCHES: &str = "Nothing matches";
const NOTHING_MATCHES_HINT: &str = "Try fewer letters.";
const ABOUT_MEDIA: &str = "About this media";

#[derive(Debug, Clone, PartialEq, Eq)]
enum Row {
    Media {
        /// `None` is the group of documents with no media.
        title: Option<String>,
        label: String,
        category: Option<Category>,
        count: usize,
        is_open: bool,
    },
    Document {
        doc: DocId,
        label: String,
        is_open: bool,
    },
}

/// The rows of the shelf, kept between frames so that drawing builds no text. They are made again
/// when the catalogue, the open item or the find text changes.
#[derive(Debug, Default)]
pub(super) struct Rows {
    key: Option<(u64, Option<Open>, String)>,
    rows: Vec<Row>,
}

impl Rows {
    pub(super) fn refresh(
        &mut self,
        catalogue: &Catalogue,
        revision: u64,
        open: Option<&Open>,
        find: &str,
    ) {
        let is_current = self.key.as_ref().is_some_and(|(seen, kept, typed)| {
            *seen == revision && kept.as_ref() == open && typed == find
        });
        if is_current {
            return;
        }
        self.rows = rows_of(catalogue, open, find);
        self.key = Some((revision, open.cloned(), find.to_owned()));
    }
}

/// The rows the shelf shows. With no find text it is every media, and the documents of the media
/// that holds the open item. With find text it is every media whose title holds the text, with all
/// its documents, and every other media with a document whose name holds it, with only those.
fn rows_of(catalogue: &Catalogue, open: Option<&Open>, find: &str) -> Vec<Row> {
    let needle = find.trim().to_lowercase();
    let mut rows = Vec::new();
    for media in &catalogue.media {
        let Some(documents) = unfolded_documents(media, open, &needle) else {
            continue;
        };
        rows.push(Row::Media {
            title: media.title.clone(),
            label: media.title.clone().unwrap_or_else(|| NO_MEDIA.to_owned()),
            category: media.title.is_some().then_some(media.category),
            count: media.documents.len(),
            is_open: matches!(open, Some(Open::Media { title }) if *title == media.title),
        });
        rows.extend(documents.into_iter().map(|document| Row::Document {
            doc: document.id,
            label: document.name().label(),
            is_open: open == Some(&Open::Document(document.id)),
        }));
    }
    rows
}

/// The documents listed under `media`, or `None` when the media has no row. `needle` is the find
/// text, trimmed and in lower case.
fn unfolded_documents<'a>(
    media: &'a Media,
    open: Option<&Open>,
    needle: &str,
) -> Option<Vec<&'a Document>> {
    if needle.is_empty() {
        let is_unfolded = match open {
            Some(Open::Media { title }) => *title == media.title,
            Some(Open::Document(doc)) => media.documents.iter().any(|document| document.id == *doc),
            None => false,
        };
        let documents = if is_unfolded {
            media.documents.iter().collect()
        } else {
            Vec::new()
        };
        return Some(documents);
    }
    let is_title_found = media
        .title
        .as_deref()
        .is_some_and(|title| title.to_lowercase().contains(needle));
    if is_title_found {
        return Some(media.documents.iter().collect());
    }
    let found: Vec<&Document> = media
        .documents
        .iter()
        .filter(|document| document.name().label().to_lowercase().contains(needle))
        .collect();
    (!found.is_empty()).then_some(found)
}

pub(super) fn show(
    ui: &mut egui::Ui,
    local: &mut Local,
    catalogue: &Catalogue,
    fit: ShelfFit,
    cx: &mut PanelCx<'_>,
) {
    ui.label(TextRole::Title.rich(ui, TITLE));
    ui.add_space(space::MD);
    let has_text = !local.find.is_empty();
    let mut find = TextInput::new(FIND, &mut local.find)
        .icon(Icon::SEARCH)
        .placeholder(FIND_HINT)
        .size(fit.find_size());
    if has_text {
        find = find.trailing(Icon::CLOSE, CLEAR);
    }
    if find.show(ui).trailing_clicked {
        local.find.clear();
    }
    ui.add_space(space::SM);

    let shared = cx.shared;
    let open = shared.shelf.open.as_ref();
    local
        .rows
        .refresh(catalogue, shared.library.revision, open, &local.find);
    if local.rows.rows.is_empty() {
        Placeholder::empty(Icon::SEARCH, NOTHING_MATCHES)
            .hint(NOTHING_MATCHES_HINT)
            .show(ui);
        return;
    }
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for row in &local.rows.rows {
                if let Some(intent) = show_row(ui, row, fit.row_size()) {
                    cx.intents.push(intent);
                }
                // Alone in the room, the open media has no page beside the shelf, so a row under it
                // opens its page. The group with no media has nothing to manage.
                if fit == ShelfFit::Alone
                    && let Row::Media {
                        title: Some(title),
                        is_open: true,
                        ..
                    } = row
                    && about_row(ui, fit.row_size())
                {
                    local.media_page = Some(title.clone());
                }
            }
        });
}

/// The row that opens the page of the open media. Returns true when it was pressed.
fn about_row(ui: &mut egui::Ui, size: ControlSize) -> bool {
    let row = ListRow::new(ABOUT_MEDIA)
        .glyph(Icon::INFO)
        .role(TextRole::Small)
        .indent(space::XL)
        .size(size);
    ui.add(row).clicked()
}

/// The intent of the row if it was clicked.
fn show_row(ui: &mut egui::Ui, row: &Row, size: ControlSize) -> Option<Intent> {
    match row {
        Row::Media {
            title,
            label,
            category,
            count,
            is_open,
        } => {
            let mut media = ListRow::new(label)
                .count(*count)
                .selected(*is_open)
                .size(size);
            if let Some(category) = category {
                media = media.glyph(category_icon(*category));
            }
            ui.add(media).clicked().then(|| Intent::OpenMedia {
                title: title.clone(),
            })
        }
        Row::Document {
            doc,
            label,
            is_open,
        } => {
            let document = ListRow::new(label)
                .role(TextRole::Small)
                .indent(space::XL)
                .selected(*is_open)
                .size(size);
            ui.add(document)
                .clicked()
                .then_some(Intent::OpenDocument(*doc))
        }
    }
}
