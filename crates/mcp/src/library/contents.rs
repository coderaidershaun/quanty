//! The `chapter_contents` tool: every formula, figure, table and code listing of one converted
//! chapter, in reading order.

use ocr::{ChapterItem, PieceDetail, read_chapter};
use rag_core::{Config, ItemKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::LibraryError;
use super::chapter::{DocumentHeader, find_chapter, parse_document_id};
use super::piece::PieceView;
use crate::args::non_blank;

/// What an agent gives `chapter_contents`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct ChapterContentsArgs {
    /// From `list_documents` or a search result.
    document_id: String,
    /// `formula`, `figure`, `table` or `code`. Every kind when left out.
    kind: Option<String>,
}

/// The formulas, figures, tables and code listings of one chapter, in reading order.
#[derive(Serialize, JsonSchema)]
pub(crate) struct ContentsView {
    #[serde(flatten)]
    document: DocumentHeader,
    /// How many items of each kind the whole chapter has, whatever `kind` asked for.
    counts: ContentsCounts,
    items: Vec<ContentsItem>,
}

#[derive(Serialize, JsonSchema)]
struct ContentsCounts {
    formula: usize,
    figure: usize,
    table: usize,
    code: usize,
}

/// One item, with the piece it starts at.
#[derive(Serialize, JsonSchema)]
struct ContentsItem {
    /// The place of the page the item starts on in the chapter, from 1. Give it to `read_page` to
    /// read around the item.
    page: u32,
    /// The page number as printed in the book. Left out when the page shows none.
    #[serde(skip_serializing_if = "Option::is_none")]
    printed_page: Option<String>,
    /// The place of the page where the item ends, from 1. Only a code listing that page breaks
    /// cut has one: it is one item, and `content` holds all of it.
    #[serde(skip_serializing_if = "Option::is_none")]
    last_page: Option<u32>,
    /// The page number as printed on the page where the item ends.
    #[serde(skip_serializing_if = "Option::is_none")]
    last_printed_page: Option<String>,
    #[serde(flatten)]
    piece: PieceView,
}

impl From<&ChapterItem<'_>> for ContentsItem {
    fn from(item: &ChapterItem<'_>) -> ContentsItem {
        ContentsItem {
            page: item.first.id.page,
            printed_page: item.first.printed_page_number.clone(),
            last_page: item.last_page(),
            last_printed_page: item.last_printed_page().map(str::to_owned),
            piece: PieceView::of_item(item),
        }
    }
}

impl ContentsCounts {
    fn of(items: &[ChapterItem<'_>]) -> ContentsCounts {
        let count = |kind: ItemKind| items.iter().filter(|item| is_of_kind(item, kind)).count();
        ContentsCounts {
            formula: count(ItemKind::Formula),
            figure: count(ItemKind::Figure),
            table: count(ItemKind::Table),
            code: count(ItemKind::Code),
        }
    }
}

fn is_of_kind(item: &ChapterItem<'_>, kind: ItemKind) -> bool {
    let item_kind = match item.first.detail {
        PieceDetail::Formula { .. } => ItemKind::Formula,
        PieceDetail::Figure { .. } => ItemKind::Figure,
        PieceDetail::Table { .. } => ItemKind::Table,
        PieceDetail::Code { .. } => ItemKind::Code,
        PieceDetail::Heading { .. } | PieceDetail::Text { .. } | PieceDetail::Footnote { .. } => {
            return false;
        }
    };
    item_kind == kind
}

/// The kind that was asked for, or `None` for every kind.
fn wanted_kind(kind: Option<String>) -> Result<Option<ItemKind>, LibraryError> {
    let kind = non_blank(kind)
        .map(|kind| kind.parse::<ItemKind>())
        .transpose()
        .map_err(LibraryError::ItemKind)?;
    match kind {
        Some(ItemKind::Chunk) => Err(LibraryError::ChunkKind),
        other => Ok(other),
    }
}

/// The items of the chapter that the document id names, of one kind or of every kind. It needs no
/// store.
///
/// # Errors
/// A bad argument, a document that no converted chapter has or whose chapter may be one that
/// cannot be read, or a chapter that cannot be read.
pub(crate) fn chapter_contents(
    config: &Config,
    args: ChapterContentsArgs,
) -> Result<ContentsView, LibraryError> {
    let id = parse_document_id(&args.document_id)?;
    let kind = wanted_kind(args.kind)?;
    let entry = find_chapter(&config.content_folder, id)?;
    let chapter = read_chapter(&entry.folder)?;
    let all = chapter.items();
    let items = all
        .iter()
        .filter(|item| kind.is_none_or(|kind| is_of_kind(item, kind)))
        .map(ContentsItem::from)
        .collect();
    Ok(ContentsView {
        document: DocumentHeader::of(&entry),
        counts: ContentsCounts::of(&all),
        items,
    })
}
