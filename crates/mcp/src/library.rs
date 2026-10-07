//! The `list_documents` and `read_page` tools: what the library holds, and the pieces of one page
//! of a converted chapter.

use std::collections::HashMap;
use std::path::PathBuf;

use graph::{FalkorGraph, GraphError, GraphStore};
use ocr::{
    Catalogue, ChapterEntry, ChapterPiece, ContentError, PieceDetail, ReadChapterError,
    read_chapter,
};
use rag_core::{Config, DocId, ParseDocIdError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What an agent gives `read_page`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct ReadPageArgs {
    /// The `document_id` of a search result or of `list_documents`.
    document_id: String,
    /// The `page` of a search result: the place of the page in the chapter, from 1. It is not the
    /// number printed on the page.
    page: u32,
}

/// Every stored document.
#[derive(Serialize, JsonSchema)]
pub(crate) struct Documents {
    /// Sorted by title.
    documents: Vec<DocumentView>,
}

#[derive(Serialize, JsonSchema)]
struct DocumentView {
    /// Give it to `read_page`.
    document_id: String,
    /// The title of the document: the book and the chapter.
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    book: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    author: Option<String>,
    tags: Vec<String>,
    /// The chapter number, when the converted chapter is under the content folder. A picture that
    /// stands alone has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    chapter_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    chapter_name: Option<String>,
    /// How many pages the chapter has.
    #[serde(skip_serializing_if = "Option::is_none")]
    pages: Option<u32>,
}

/// The pieces of one page, in reading order.
#[derive(Serialize, JsonSchema)]
pub(crate) struct PageView {
    document_id: String,
    book: String,
    chapter_number: u32,
    chapter_name: String,
    /// The place of the page in the chapter, from 1.
    page: u32,
    /// How many pages the chapter has.
    pages: u32,
    /// The page number as printed in the book. Left out when the page shows none.
    #[serde(skip_serializing_if = "Option::is_none")]
    printed_page: Option<String>,
    /// The path of the picture of the whole page, on the disk of the machine the server runs on.
    #[serde(skip_serializing_if = "Option::is_none")]
    page_image: Option<String>,
    pieces: Vec<PieceView>,
}

#[derive(Serialize, JsonSchema)]
struct PieceView {
    /// The place of the piece on the page, from 1.
    number: u32,
    /// `heading`, `text`, `formula`, `figure`, `table` or `footnote`.
    kind: String,
    /// The headings the piece sits under, outermost first.
    section: Vec<String>,
    /// The label as printed, such as "(7.3)". Formulas, figures and tables only.
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    /// The piece as saved: Markdown, or LaTeX for a formula.
    content: String,
    /// The path of the picture of a figure, on the disk of the machine the server runs on.
    #[serde(skip_serializing_if = "Option::is_none")]
    picture: Option<String>,
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum LibraryError {
    #[error("`document_id` is not valid; take it from a search result or from `list_documents`")]
    DocumentId(#[from] ParseDocIdError),

    #[error("`page` is 0; pages count from 1")]
    ZeroPage,

    #[error("could not read the documents from the graph")]
    Graph(#[from] GraphError),

    #[error(
        "could not list the converted chapters; check that the server can read its content folder"
    )]
    Content(#[from] ContentError),

    #[error(
        "no converted chapter under {} has the document id {id}; call `list_documents` to see the document ids",
        folder.display()
    )]
    UnknownDocument { id: DocId, folder: PathBuf },

    #[error("could not read the converted chapter")]
    Chapter(#[from] ReadChapterError),

    #[error("page {page} is out of range: the chapter has {pages} pages, numbered from 1")]
    PageOutOfRange { page: u32, pages: u32 },
}

/// Every document of the graph with its labels, and its chapter when that is under the content
/// folder.
///
/// # Errors
/// The graph that is not ready, or a content folder that cannot be listed.
pub(crate) async fn list_documents(config: &Config) -> Result<Documents, LibraryError> {
    let graph = FalkorGraph::connect(config).await?;
    let nodes = graph.documents().await?;
    // SMELL: a chapter whose `chapter.json` cannot be read is passed over without a word, here
    // and in `read_page`. Its document then shows no chapter, and `read_page` says that it has
    // no converted chapter.
    let catalogue = Catalogue::read(&config.content_folder)?;
    let chapters: HashMap<DocId, &ChapterEntry> = catalogue
        .chapters
        .iter()
        .map(|entry| (document_of(entry), entry))
        .collect();
    let mut documents: Vec<DocumentView> = nodes
        .into_iter()
        .map(|node| {
            let chapter = chapters.get(&node.id).map(|entry| &entry.index);
            DocumentView {
                document_id: node.id.to_string(),
                title: node.title,
                book: node.labels.book,
                author: node.labels.author,
                tags: node.labels.tags.iter().map(ToString::to_string).collect(),
                chapter_number: chapter.map(|index| index.chapter_number),
                chapter_name: chapter.map(|index| index.chapter_name.clone()),
                pages: chapter.map(|index| index.page_count),
            }
        })
        .collect();
    documents.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(Documents { documents })
}

/// The pieces of one page of the chapter that the document id names. It needs no store.
///
/// # Errors
/// A bad argument, a document that no converted chapter has, a page that the chapter does not
/// have, or a chapter that cannot be read.
pub(crate) fn read_page(config: &Config, args: ReadPageArgs) -> Result<PageView, LibraryError> {
    let id: DocId = args.document_id.trim().parse()?;
    if args.page == 0 {
        return Err(LibraryError::ZeroPage);
    }
    let catalogue = Catalogue::read(&config.content_folder)?;
    let entry = catalogue
        .chapters
        .iter()
        .find(|entry| document_of(entry) == id)
        .ok_or_else(|| LibraryError::UnknownDocument {
            id,
            folder: config.content_folder.clone(),
        })?;
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
        document_id: id.to_string(),
        book: entry.index.book_title.clone(),
        chapter_number: entry.index.chapter_number,
        chapter_name: entry.index.chapter_name.clone(),
        page: args.page,
        pages: entry.index.page_count,
        printed_page: on_page
            .first()
            .and_then(|piece| piece.printed_page_number.clone()),
        page_image: on_page
            .first()
            .map(|piece| piece.page_image.display().to_string()),
        pieces: on_page.into_iter().map(PieceView::from).collect(),
    })
}

impl From<&ChapterPiece> for PieceView {
    fn from(piece: &ChapterPiece) -> PieceView {
        PieceView {
            number: piece.id.number,
            kind: piece.detail.kind_name().to_owned(),
            section: piece
                .section
                .iter()
                .map(|heading| heading.text.clone())
                .collect(),
            label: label_of(&piece.detail),
            content: piece.content.clone(),
            picture: piece
                .figure_image
                .as_ref()
                .map(|picture| picture.path.display().to_string()),
        }
    }
}

/// The document that a converted chapter is stored as: the same PDF always gives the same id.
fn document_of(entry: &ChapterEntry) -> DocId {
    DocId::from_source_sha256(&entry.index.source_sha256)
}

fn label_of(detail: &PieceDetail) -> Option<String> {
    match detail {
        PieceDetail::Formula { label, .. }
        | PieceDetail::Figure { label, .. }
        | PieceDetail::Table { label, .. } => label.clone(),
        PieceDetail::Heading { .. } | PieceDetail::Text { .. } | PieceDetail::Footnote { .. } => {
            None
        }
    }
}
