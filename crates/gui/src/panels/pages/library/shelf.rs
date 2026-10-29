//! The shelf: the box that finds a media or a chapter, the rows of the media with the documents
//! of the open one, and the edge that is dragged to make it wider or narrower.

use eframe::egui;

use super::{Local, ShelfFit, TITLE};
use crate::panels::PanelCx;
use crate::panels::labels::NO_MEDIA;
use crate::panels::pages::media_card::category_icon;
use crate::state::{Open, Shelf};
use crate::theme::{self, Icon, TextRole, space, stroke};
use crate::widgets::{ControlSize, ListRow, Placeholder, TextInput};
use contract::{Catalogue, Category, DocId, Document, Intent, Media};

const FIND: &str = "Find";
const FIND_HINT: &str = "Find a media or chapter";
const CLEAR: &str = "Clear";
const NOTHING_MATCHES: &str = "Nothing matches";
const NOTHING_MATCHES_HINT: &str = "Try fewer letters.";
const ABOUT_MEDIA: &str = "About this media";
const EDGE: &str = "Resize the shelf";

/// The narrowest and the widest the shelf can be dragged to. The widest is cut further so that the
/// page keeps `MIN_PAGE_WIDTH`.
const MIN_WIDTH: f32 = 200.0;
const MAX_WIDTH: f32 = 520.0;
const MIN_PAGE_WIDTH: f32 = 400.0;

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
/// when the catalogue, the open item, its fold or the find text changes.
#[derive(Debug, Default)]
pub(super) struct Rows {
    key: Option<(u64, Option<Open>, bool, String)>,
    rows: Vec<Row>,
}

impl Rows {
    pub(super) fn refresh(
        &mut self,
        catalogue: &Catalogue,
        revision: u64,
        shelf: &Shelf,
        find: &str,
    ) {
        let is_current = self
            .key
            .as_ref()
            .is_some_and(|(seen, kept, is_folded, typed)| {
                *seen == revision
                    && *kept == shelf.open
                    && *is_folded == shelf.is_folded
                    && typed == find
            });
        if is_current {
            return;
        }
        self.rows = rows_of(catalogue, shelf, find);
        self.key = Some((
            revision,
            shelf.open.clone(),
            shelf.is_folded,
            find.to_owned(),
        ));
    }
}

/// The rows the shelf shows. With no find text it is every media, and the documents of the media
/// that holds the open item, unless that item is the media and it is folded. With find text it is
/// every media whose title holds the text, with all its documents, and every other media with a
/// document whose name holds it, with only those.
fn rows_of(catalogue: &Catalogue, shelf: &Shelf, find: &str) -> Vec<Row> {
    let needle = find.trim().to_lowercase();
    let open = shelf.open.as_ref();
    let mut rows = Vec::new();
    for media in &catalogue.media {
        let Some(documents) = unfolded_documents(media, shelf, &needle) else {
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
    shelf: &Shelf,
    needle: &str,
) -> Option<Vec<&'a Document>> {
    if needle.is_empty() {
        let is_unfolded = match &shelf.open {
            Some(Open::Media { title }) => *title == media.title && !shelf.is_folded,
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
    local.rows.refresh(
        catalogue,
        shared.library.revision,
        &shared.shelf,
        &local.find,
    );
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

/// How wide the shelf is in a room `room_width` wide: as the person dragged it, or else `suited`,
/// kept within the widths it can be dragged to.
pub(super) fn width(local: &Local, suited: f32, room_width: f32) -> f32 {
    local
        .shelf_width
        .unwrap_or(suited)
        .clamp(MIN_WIDTH, widest(room_width))
}

fn widest(room_width: f32) -> f32 {
    MAX_WIDTH
        .min(room_width - MIN_PAGE_WIDTH - space::MD)
        .max(MIN_WIDTH)
}

/// The strip between the shelf and the page is a handle: dragging it sets the width of the shelf,
/// and the edge shows under the pointer.
pub(super) fn drag_edge(ui: &mut egui::Ui, shelf: egui::Rect, room_width: f32, local: &mut Local) {
    let handle = egui::Rect::from_min_max(
        shelf.right_top(),
        egui::pos2(shelf.right() + space::MD, shelf.bottom()),
    );
    let response = ui
        .interact(handle, ui.id().with("shelf_edge"), egui::Sense::drag())
        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(egui::accesskit::Role::Splitter);
        node.set_label(EDGE);
    });
    if let Some(pointer) = response.interact_pointer_pos()
        && response.dragged()
    {
        // The pointer is taken to hold the middle of the gap, so a press that does not move keeps
        // the width.
        let width = pointer.x - shelf.left() - space::MD / 2.0;
        local.shelf_width = Some(width.clamp(MIN_WIDTH, widest(room_width)));
    }
    if response.hovered() || response.dragged() {
        let palette = theme::palette(ui.ctx());
        let color = if response.dragged() {
            palette.focus
        } else {
            palette.border
        };
        let x = handle.center().x;
        ui.painter().line_segment(
            [egui::pos2(x, handle.top()), egui::pos2(x, handle.bottom())],
            egui::Stroke::new(stroke::BORDER, color),
        );
    }
}
