//! The `chapter_contents` tool: every formula, figure, table and code listing of one converted
//! chapter, in reading order.

use ocr::{ChapterItem, PieceDetail, read_chapter};
use rag_core::{Config, ItemKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::LibraryError;
use super::chapter::{DocumentHeader, find_chapter, parse_document_id};
use super::piece::PieceView;
use crate::args::{Budget, Scope, Truncation, parse_optional};

/// What an agent gives `chapter_contents`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct ChapterContentsArgs {
    /// From `list_documents` or a search result.
    document_id: String,
    /// `formula`, `figure`, `table` or `code`. Every kind when left out.
    kind: Option<String>,
    /// At most this many characters of text. 24000 when left out.
    max_chars: Option<u32>,
    /// From a result that was `truncated`, with the same other arguments.
    cursor: Option<String>,
}

/// The formulas, figures, tables and code listings of one chapter, in reading order.
#[derive(Serialize, JsonSchema)]
pub(crate) struct ContentsView {
    #[serde(flatten)]
    document: DocumentHeader,
    /// How many items of each kind the whole chapter has, whatever `kind` asked for.
    counts: ContentsCounts,
    items: Vec<ContentsItem>,
    #[serde(flatten)]
    more: Truncation,
}

#[derive(Serialize, JsonSchema)]
pub(super) struct ContentsCounts {
    formula: usize,
    figure: usize,
    table: usize,
    code: usize,
}

/// One item, with the piece it starts at.
#[derive(Serialize, JsonSchema)]
pub(super) struct ContentsItem {
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

impl ContentsItem {
    pub(super) fn from_item(
        item: &ChapterItem<'_>,
        content: &std::path::Path,
        shown_content: String,
    ) -> ContentsItem {
        ContentsItem {
            page: item.first.id.page,
            printed_page: item.first.printed_page_number.clone(),
            last_page: item.last_page(),
            last_printed_page: item.last_printed_page().map(str::to_owned),
            piece: PieceView::of_item(item, content).with_content(shown_content),
        }
    }
}

impl ContentsCounts {
    pub(super) fn of(items: &[ChapterItem<'_>]) -> ContentsCounts {
        let count = |kind: ItemKind| items.iter().filter(|item| is_of_kind(item, kind)).count();
        ContentsCounts {
            formula: count(ItemKind::Formula),
            figure: count(ItemKind::Figure),
            table: count(ItemKind::Table),
            code: count(ItemKind::Code),
        }
    }

    /// How many of each kind these are. A chunk is not counted.
    pub(super) fn of_kinds(kinds: impl IntoIterator<Item = ItemKind>) -> ContentsCounts {
        let mut counts = ContentsCounts {
            formula: 0,
            figure: 0,
            table: 0,
            code: 0,
        };
        for kind in kinds {
            match kind {
                ItemKind::Formula => counts.formula += 1,
                ItemKind::Figure => counts.figure += 1,
                ItemKind::Table => counts.table += 1,
                ItemKind::Code => counts.code += 1,
                ItemKind::Chunk => {}
            }
        }
        counts
    }
}

fn is_of_kind(item: &ChapterItem<'_>, kind: ItemKind) -> bool {
    kind_of(item) == Some(kind)
}

/// The kind of an item of a chapter, which is never a chunk.
pub(super) fn kind_of(item: &ChapterItem<'_>) -> Option<ItemKind> {
    match item.first.detail {
        PieceDetail::Formula { .. } => Some(ItemKind::Formula),
        PieceDetail::Figure { .. } => Some(ItemKind::Figure),
        PieceDetail::Table { .. } => Some(ItemKind::Table),
        PieceDetail::Code { .. } => Some(ItemKind::Code),
        PieceDetail::Heading { .. } | PieceDetail::Text { .. } | PieceDetail::Footnote { .. } => {
            None
        }
    }
}

/// The kind that was asked for, or `None` for every kind.
fn wanted_kind(kind: Option<String>) -> Result<Option<ItemKind>, LibraryError> {
    let kind = parse_optional::<ItemKind>(kind).map_err(LibraryError::ItemKind)?;
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
    let scope = Scope::of("chapter_contents")
        .with(id)
        .with(kind.map_or("every kind", |kind| kind.as_str()));
    let budget = Budget::new(scope, args.max_chars, args.cursor.as_deref())?;
    let chapter = read_chapter(&entry.folder)?;
    let all = chapter.items();
    let wanted: Vec<&ChapterItem<'_>> = all
        .iter()
        .filter(|item| kind.is_none_or(|kind| is_of_kind(item, kind)))
        .collect();
    let contents: Vec<String> = wanted.iter().map(|item| item.content()).collect();
    let taken = budget.take(&contents)?;
    let items = taken
        .slices
        .into_iter()
        .map(|slice| {
            ContentsItem::from_item(wanted[slice.unit], &config.content_folder, slice.text)
        })
        .collect();
    Ok(ContentsView {
        document: DocumentHeader::of(&entry),
        counts: ContentsCounts::of(&all),
        items,
        more: taken.truncation,
    })
}
