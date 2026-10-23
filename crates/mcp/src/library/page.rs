//! The `read_page` tool: the pieces of one page of a converted chapter, in reading order.

use ocr::read_chapter;
use rag_core::Config;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::LibraryError;
use super::chapter::{DocumentHeader, find_chapter, parse_document_id};
use super::piece::PieceView;

/// What an agent gives `read_page`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct ReadPageArgs {
    /// The `document_id` of a search result or of `list_documents`.
    document_id: String,
    /// The `page` of a search result: the place of the page in the chapter, from 1. It is not the
    /// number printed on the page.
    page: u32,
}

/// The pieces of one page, in reading order.
#[derive(Serialize, JsonSchema)]
pub(crate) struct PageView {
    #[serde(flatten)]
    document: DocumentHeader,
    /// The place of the page in the chapter, from 1.
    page: u32,
    /// The page number as printed in the book. Left out when the page shows none.
    #[serde(skip_serializing_if = "Option::is_none")]
    printed_page: Option<String>,
    /// The whole-page picture URL relative to the app base, contained beneath content/.
    #[serde(skip_serializing_if = "Option::is_none")]
    page_image: Option<String>,
    pieces: Vec<PieceView>,
}

/// The pieces of one page of the chapter that the document id names. It needs no store.
///
/// # Errors
/// A bad argument, a document that no converted chapter has or whose chapter may be one that
/// cannot be read, a page that the chapter does not have, or a chapter that cannot be read.
pub(crate) fn read_page(config: &Config, args: ReadPageArgs) -> Result<PageView, LibraryError> {
    let id = parse_document_id(&args.document_id)?;
    if args.page == 0 {
        return Err(LibraryError::ZeroPage);
    }
    let entry = find_chapter(&config.content_folder, id)?;
    if args.page > entry.index.page_count {
        return Err(LibraryError::PageOutOfRange {
            page: args.page,
            pages: entry.index.page_count,
        });
    }
    let chapter = read_chapter(&entry.folder)?;
    let on_page: Vec<_> = chapter
        .pieces
        .iter()
        .filter(|piece| piece.id.page == args.page)
        .collect();
    Ok(PageView {
        document: DocumentHeader::of(&entry),
        page: args.page,
        printed_page: on_page
            .first()
            .and_then(|piece| piece.printed_page_number.clone()),
        page_image: on_page
            .first()
            .and_then(|piece| rag_core::content_url(&piece.page_image, &config.content_folder)),
        pieces: on_page
            .into_iter()
            .map(|piece| PieceView::from_piece(piece, &config.content_folder))
            .collect(),
    })
}
