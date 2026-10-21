//! The shelf: the box that finds a media or a chapter, and the rows of the media with the
//! documents of the open one.

use eframe::egui;

use super::{Local, TITLE};
use crate::contract::{Catalogue, Category, DocId, Document, Intent, Media};
use crate::panels::PanelCx;
use crate::panels::media_card::{NO_MEDIA, category_icon};
use crate::state::Open;
use crate::theme::{Icon, TextRole, space};
use crate::widgets::{ListRow, Placeholder, TextInput};

const FIND: &str = "Find";
const FIND_HINT: &str = "Find a media or chapter";
const CLEAR: &str = "Clear";
const NOTHING_MATCHES: &str = "Nothing matches";
const NOTHING_MATCHES_HINT: &str = "Try fewer letters.";

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
    cx: &mut PanelCx<'_>,
) {
    ui.label(TextRole::Title.rich(TITLE));
    ui.add_space(space::MD);
    let has_text = !local.find.is_empty();
    let mut find = TextInput::new(FIND, &mut local.find)
        .icon(Icon::SEARCH)
        .placeholder(FIND_HINT);
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
                if let Some(intent) = show_row(ui, row) {
                    cx.intents.push(intent);
                }
            }
        });
}

/// The intent of the row if it was clicked.
fn show_row(ui: &mut egui::Ui, row: &Row) -> Option<Intent> {
    match row {
        Row::Media {
            title,
            label,
            category,
            count,
            is_open,
        } => {
            let mut media = ListRow::new(label).count(*count).selected(*is_open);
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
                .selected(*is_open);
            ui.add(document)
                .clicked()
                .then_some(Intent::OpenDocument(*doc))
        }
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::contract::ChapterLabel;

    fn chapter(id: u128, number: u32, name: &str) -> Document {
        Document {
            id: DocId(Uuid::from_u128(id)),
            chapter: Some(ChapterLabel {
                number,
                name: name.to_owned(),
            }),
            ..Document::default()
        }
    }

    fn titled(id: u128, title: &str) -> Document {
        Document {
            id: DocId(Uuid::from_u128(id)),
            title: title.to_owned(),
            ..Document::default()
        }
    }

    fn media(title: Option<&str>, category: Category, documents: Vec<Document>) -> Media {
        Media {
            title: title.map(str::to_owned),
            category,
            documents,
            ..Media::default()
        }
    }

    /// A paper, a book and a book of notes with chapters, and the documents of no media.
    fn library() -> Catalogue {
        Catalogue {
            media: vec![
                media(
                    Some("Hawkes Processes in Finance"),
                    Category::Paper,
                    vec![titled(10, "Self-Exciting Order Flow")],
                ),
                media(
                    Some("Option Volatility and Pricing"),
                    Category::Book,
                    vec![
                        chapter(1, 1, "Sample Pages"),
                        chapter(2, 2, "Greeks"),
                        titled(3, "Index"),
                    ],
                ),
                media(
                    Some("Quanty Sample Notes"),
                    Category::Book,
                    vec![
                        chapter(4, 1, "Options Pricing Intuition"),
                        chapter(5, 2, "Black Scholes In Depth"),
                        titled(6, "Notes Index"),
                    ],
                ),
                media(
                    None,
                    Category::Book,
                    vec![titled(11, "A picture on its own")],
                ),
            ],
        }
    }

    fn open_media(title: &str) -> Open {
        Open::Media {
            title: Some(title.to_owned()),
        }
    }

    fn open_document(number: u128) -> Open {
        Open::Document(DocId(Uuid::from_u128(number)))
    }

    /// One line for each row, a media with its count, a document indented, `>` before the open row.
    fn lines(rows: &[Row]) -> Vec<String> {
        let mark = |is_open: bool| if is_open { "> " } else { "" };
        rows.iter()
            .map(|row| match row {
                Row::Media {
                    label,
                    count,
                    is_open,
                    ..
                } => format!("{}{label} ({count})", mark(*is_open)),
                Row::Document { label, is_open, .. } => format!("    {}{label}", mark(*is_open)),
            })
            .collect()
    }

    #[test]
    fn every_media_is_a_row_and_only_the_open_media_unfolds() {
        let library = library();
        assert_eq!(
            lines(&rows_of(&library, None, "")),
            [
                "Hawkes Processes in Finance (1)",
                "Option Volatility and Pricing (3)",
                "Quanty Sample Notes (3)",
                "No media (1)",
            ]
        );

        let book = open_media("Option Volatility and Pricing");
        assert_eq!(
            lines(&rows_of(&library, Some(&book), "")),
            [
                "Hawkes Processes in Finance (1)",
                "> Option Volatility and Pricing (3)",
                "    Chapter 1 · Sample Pages",
                "    Chapter 2 · Greeks",
                "    Index",
                "Quanty Sample Notes (3)",
                "No media (1)",
            ]
        );

        let chapter = open_document(5);
        assert_eq!(
            lines(&rows_of(&library, Some(&chapter), "")),
            [
                "Hawkes Processes in Finance (1)",
                "Option Volatility and Pricing (3)",
                "Quanty Sample Notes (3)",
                "    Chapter 1 · Options Pricing Intuition",
                "    > Chapter 2 · Black Scholes In Depth",
                "    Notes Index",
                "No media (1)",
            ]
        );

        let group = Open::Media { title: None };
        let rows = rows_of(&library, Some(&group), "");
        assert_eq!(
            lines(&rows)[3..],
            ["> No media (1)", "    A picture on its own"]
        );
        let glyphs: Vec<bool> = rows
            .iter()
            .filter_map(|row| match row {
                Row::Media { category, .. } => Some(category.is_some()),
                Row::Document { .. } => None,
            })
            .collect();
        assert_eq!(
            glyphs,
            [true, true, true, false],
            "the group has no category"
        );
    }

    #[test]
    fn the_find_text_unfolds_a_matching_media_whole_and_another_media_to_its_matching_documents() {
        let library = library();
        assert_eq!(
            lines(&rows_of(&library, None, "  OPTION ")),
            [
                "Option Volatility and Pricing (3)",
                "    Chapter 1 · Sample Pages",
                "    Chapter 2 · Greeks",
                "    Index",
                "Quanty Sample Notes (3)",
                "    Chapter 1 · Options Pricing Intuition",
            ],
            "a title unfolds its media whole, and a document unfolds only itself"
        );

        let chapter = open_document(2);
        assert_eq!(
            lines(&rows_of(&library, Some(&chapter), "chapter 2")),
            [
                "Option Volatility and Pricing (3)",
                "    > Chapter 2 · Greeks",
                "Quanty Sample Notes (3)",
                "    Chapter 2 · Black Scholes In Depth",
            ]
        );

        assert!(rows_of(&library, None, "zzz").is_empty());
        let book = open_media("Hawkes Processes in Finance");
        assert_eq!(
            rows_of(&library, Some(&book), "   "),
            rows_of(&library, Some(&book), ""),
            "blank text gives the normal rows"
        );
    }
}
