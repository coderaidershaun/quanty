//! The `read_item` tool: one formula, figure, table or code listing of a converted chapter, by the
//! label the book prints or by its item id, with the paragraphs that cite it.

use ocr::{Catalogue, ChapterItem, ChapterPiece, CiteKind, PieceDetail, read_chapter};
use rag_core::{Config, DocId, ItemId, ItemKind};
use rag_ingestion::{Item, chapter_items};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::args::{Budget, Scope, Truncation, non_blank, parse_optional};
use crate::library::LibraryError;
use crate::library::chapter::{DocumentHeader, Whole, parse_document_id};
use crate::library::contents::{ContentsItem, kind_of};
use crate::library::piece::label_of;

/// How many characters of a citing paragraph `cited_by` shows.
const EXCERPT_CHARS: usize = 200;

/// What an agent gives `read_item`: the `document_id`, `kind` and `label` of the item, or its
/// `item_id`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct ReadItemArgs {
    /// With `kind` and `label`. With `item_id` it is the document to look in, which is faster.
    pub(crate) document_id: Option<String>,
    /// `formula`, `figure`, `table` or `code`. With `label`.
    pub(crate) kind: Option<String>,
    /// The label as printed, such as "(7.3)" or "Figure 13-4". With `kind`.
    pub(crate) label: Option<String>,
    /// The `item_id` of a search result or of `read_section`. Instead of `kind` and `label`.
    pub(crate) item_id: Option<String>,
    /// At most this many characters of content. 24000 when left out.
    pub(crate) max_chars: Option<u32>,
    /// From a result that was `truncated`, with the same other arguments.
    pub(crate) cursor: Option<String>,
}

/// One item of a document, whole.
#[derive(Serialize, JsonSchema)]
pub(crate) struct ItemRead {
    #[serde(flatten)]
    document: DocumentHeader,
    item_id: String,
    #[serde(flatten)]
    item: ContentsItem,
    /// The `index` of the section that holds the item, as `document_outline` gives it.
    #[serde(skip_serializing_if = "Option::is_none")]
    section_index: Option<usize>,
    /// The text and footnotes of the document that cite the item by its label.
    cited_by: Vec<CitedBy>,
    #[serde(flatten)]
    more: Truncation,
}

impl ItemRead {
    pub(crate) fn truncation(&self) -> &Truncation {
        &self.more
    }
}

#[derive(Serialize, JsonSchema)]
struct CitedBy {
    page: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    printed_page: Option<String>,
    /// The place of the piece on the page, from 1.
    number: u32,
    /// The first 200 characters of the piece.
    excerpt: String,
}

/// The item the caller named.
enum Wanted {
    Labelled {
        document: DocId,
        kind: ItemKind,
        label: String,
    },
    Stored {
        id: ItemId,
        document: Option<DocId>,
    },
}

impl ReadItemArgs {
    fn wanted(&self) -> Result<Wanted, LibraryError> {
        let document = non_blank(self.document_id.clone())
            .map(|text| parse_document_id(&text))
            .transpose()?;
        let kind = parse_optional::<ItemKind>(self.kind.clone()).map_err(LibraryError::ItemKind)?;
        let label = non_blank(self.label.clone());
        let id = non_blank(self.item_id.clone());
        match (id, document, kind, label) {
            (Some(id), document, None, None) => Ok(Wanted::Stored {
                id: id.parse().map_err(LibraryError::ItemId)?,
                document,
            }),
            (None, Some(document), Some(kind), Some(label)) => {
                if kind == ItemKind::Chunk {
                    return Err(LibraryError::ItemKindIsText);
                }
                Ok(Wanted::Labelled {
                    document,
                    kind,
                    label,
                })
            }
            _ => Err(LibraryError::ItemChoice),
        }
    }
}

/// One item of a converted chapter. It needs no store: the item is found in the saved chapter, and
/// its id is the one the stores hold it under.
///
/// # Errors
/// A bad argument, an item that no converted chapter has, or a chapter that cannot be read.
pub(crate) fn read_item(config: &Config, args: ReadItemArgs) -> Result<ItemRead, LibraryError> {
    let wanted = args.wanted()?;
    let (whole, place) = match wanted {
        Wanted::Labelled {
            document,
            kind,
            label,
        } => {
            let whole = Whole::find(&config.content_folder, document)?;
            let place = place_of_label(&whole, kind, &label)?;
            (whole, place)
        }
        Wanted::Stored { id, document } => {
            let whole = whole_holding(config, id, document)?;
            let place = place_of_id(&whole, id)?;
            (whole, place)
        }
    };
    let items = whole.chapter.items();
    let item = &items[place];
    let id = identified(&whole, &items)[place].ok_or(LibraryError::ItemsDiffer)?;
    let scope = Scope::of("read_item").with(id);
    let budget = Budget::new(scope, args.max_chars, args.cursor.as_deref())?;
    let (shown, more) = budget.take_text(&item.content())?;
    Ok(ItemRead {
        document: DocumentHeader::of(&whole.entry),
        item_id: id.to_string(),
        item: ContentsItem::from_item(item, &config.content_folder, shown),
        section_index: whole.section_holding(id).map(|(index, _)| index),
        cited_by: cited_by(&whole, item),
        more,
    })
}

/// The place, among the items of the chapter, of the one of this kind with this label.
// SMELL: two items of one kind with the same label are not told apart; the first is read.
fn place_of_label(whole: &Whole, kind: ItemKind, label: &str) -> Result<usize, LibraryError> {
    whole
        .chapter
        .items()
        .iter()
        .position(|item| {
            kind_of(item) == Some(kind) && printed_label(item).as_deref() == Some(label)
        })
        .ok_or_else(|| LibraryError::NoItemLabelled {
            kind: kind.as_str().to_owned(),
            label: label.to_owned(),
        })
}

fn place_of_id(whole: &Whole, id: ItemId) -> Result<usize, LibraryError> {
    let items = whole.chapter.items();
    identified(whole, &items)
        .iter()
        .position(|found| *found == Some(id))
        .ok_or(LibraryError::NoSuchItem { id })
}

/// The chapter that holds the item: the document given, or the first converted chapter that
/// does.
// SMELL: without a document this reads every converted chapter until one holds the item. Item ids
// are made from the position of the item in its document, so no id says which document it is of.
fn whole_holding(
    config: &Config,
    id: ItemId,
    document: Option<DocId>,
) -> Result<Whole, LibraryError> {
    if let Some(document) = document {
        return Whole::find(&config.content_folder, document);
    }
    let catalogue = Catalogue::read(&config.content_folder)?;
    for entry in catalogue.chapters {
        let Ok(chapter) = read_chapter(&entry.folder) else {
            continue;
        };
        if chapter_items(&chapter).iter().any(|item| item.id == id) {
            return Whole::of(entry);
        }
    }
    Err(LibraryError::NoSuchItem { id })
}

/// The id that each item of the chapter is stored under, in the order of `items`. The stored
/// items and the items of the chapter are both in reading order, so they are walked together.
fn identified(whole: &Whole, items: &[ChapterItem<'_>]) -> Vec<Option<ItemId>> {
    let stored: Vec<&Item> = whole
        .items
        .iter()
        .filter(|item| item.payload.kind != ItemKind::Chunk)
        .collect();
    let mut next = 0;
    items
        .iter()
        .map(|item| {
            let found = stored[next..]
                .iter()
                .position(|candidate| is_stored_as(item, candidate))?;
            next += found + 1;
            Some(stored[next - 1].id)
        })
        .collect()
}

fn is_stored_as(item: &ChapterItem<'_>, stored: &Item) -> bool {
    kind_of(item) == Some(stored.payload.kind)
        && item.first.id.page == stored.payload.page
        && printed_label(item) == stored.payload.label
}

/// The label as the stored items keep it: trimmed, and none when blank.
fn printed_label(item: &ChapterItem<'_>) -> Option<String> {
    let label = label_of(&item.first.detail)?;
    let label = label.trim();
    (!label.is_empty()).then(|| label.to_owned())
}

/// The text and footnote pieces of the chapter that cite the item by its label.
fn cited_by(whole: &Whole, item: &ChapterItem<'_>) -> Vec<CitedBy> {
    let Some(label) = printed_label(item) else {
        return Vec::new();
    };
    whole
        .chapter
        .pieces
        .iter()
        .filter(|piece| cites(piece, &label))
        .map(|piece| CitedBy {
            page: piece.id.page,
            printed_page: piece.printed_page_number.clone(),
            number: piece.id.number,
            excerpt: excerpt_of(&piece.content),
        })
        .collect()
}

fn cites(piece: &ChapterPiece, label: &str) -> bool {
    let cites = match &piece.detail {
        PieceDetail::Text { cites } | PieceDetail::Footnote { cites, .. } => cites,
        _ => return false,
    };
    cites
        .iter()
        .any(|cite| cite.kind != CiteKind::Footnote && cite.label.trim() == label)
}

fn excerpt_of(content: &str) -> String {
    let mut excerpt: String = content.chars().take(EXCERPT_CHARS).collect();
    if content.chars().count() > EXCERPT_CHARS {
        excerpt.push('…');
    }
    excerpt
}
