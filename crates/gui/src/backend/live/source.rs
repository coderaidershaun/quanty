//! Reads one page of a saved chapter from disk, so the page shows while the stores are down, and
//! the concepts of that page from the graph.

use std::path::{Path, PathBuf};

use graph::GraphStore;
use ocr::content::{CHAPTER_INDEX_FILE, PAGE_IMAGE_FILE, PageIndex, page_folder_name};
use ocr::{Chapter, ChapterPiece, ContentError, ImageShows, PieceDetail, ReadChapterError};

use super::chapters::chapters_on_disk;
use super::{LiveContext, Services};
use crate::backend::Reply;
use crate::contract::{
    ChapterLabel, DocId, Event, Failure, FailureKind, ImageRef, PageBox, PageConcept, PagePiece,
    PageView, PieceKind, RequestId,
};

/// The page to read, and the folder of its chapter if the catalogue knows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageTarget {
    pub doc: DocId,
    pub page: u32,
    pub folder: Option<PathBuf>,
}

/// Sends the page, then the concepts of the page: always both, always in this order. The window
/// takes the concepts as the end of the load, so it would wait for ever without them, and it
/// would drop a page that came after them.
pub async fn load_page<S: Services>(
    cx: &LiveContext<S>,
    request: RequestId,
    target: &PageTarget,
    reply: &Reply,
) {
    let page = read_page(cx, target).await;
    reply.send(Event::Page {
        request,
        result: page,
    });
    let concepts = page_concepts(cx, target).await;
    reply.send(Event::PageConcepts {
        request,
        result: concepts,
    });
}

/// Why a page could not be built. It never leaves this file: it becomes a `Failure` at once.
#[derive(Debug)]
enum PageFault {
    Read(ReadChapterError),
    FolderUnknown,
    NoSuchPage { page_count: u32 },
}

impl From<ReadChapterError> for PageFault {
    fn from(error: ReadChapterError) -> PageFault {
        PageFault::Read(error)
    }
}

impl From<ContentError> for PageFault {
    fn from(error: ContentError) -> PageFault {
        PageFault::Read(ReadChapterError::Content(error))
    }
}

impl PageFault {
    fn failure<S: Services>(self, cx: &LiveContext<S>, target: &PageTarget) -> Failure {
        match self {
            PageFault::Read(error) => cx.failure(error),
            PageFault::FolderUnknown => {
                let searched = cx.config().content_folder.display();
                Failure::new(
                    FailureKind::SourceMissing,
                    format!(
                        "no chapter folder was found for document {}: none was given that holds a {CHAPTER_INDEX_FILE}, and no chapter that could be read under {searched} is its chapter",
                        target.doc.0
                    ),
                )
                .with_hint(format!(
                    "quanty does not know where this chapter's pages are. Put its chapter folder inside a book folder under {searched}, or ingest its PDF again with rag-ingest pdf."
                ))
            }
            PageFault::NoSuchPage { page_count } => Failure::new(
                FailureKind::SourceMissing,
                format!(
                    "page {} is not in the chapter of document {}, which has {page_count} pages",
                    target.page, target.doc.0
                ),
            )
            .with_hint(format!(
                "This chapter has {page_count} pages, so page {} is not in it. Run rag-ingest on its chapter folder again.",
                target.page
            )),
        }
    }
}

async fn read_page<S: Services>(
    cx: &LiveContext<S>,
    target: &PageTarget,
) -> Result<PageView, Failure> {
    let wanted = target.clone();
    let content_folder = cx.config().content_folder.clone();
    let read = tokio::task::spawn_blocking(move || page_from_disk(&wanted, &content_folder)).await;
    match read {
        Ok(Ok(page)) => Ok(page),
        Ok(Err(fault)) => Err(fault.failure(cx, target)),
        Err(error) => Err(Failure::internal(format!(
            "the page reader stopped: {error}"
        ))),
    }
}

async fn page_concepts<S: Services>(
    cx: &LiveContext<S>,
    target: &PageTarget,
) -> Result<Vec<PageConcept>, Failure> {
    let graph = cx.graph().await?;
    let concepts = graph
        .concepts_on_page(target.doc.into(), target.page)
        .await
        .map_err(|error| cx.failure(error))?;
    Ok(concepts
        .into_iter()
        .map(|node| PageConcept {
            id: node.id.into(),
            name: node.name,
            definition: node.definition,
        })
        .collect())
}

fn page_from_disk(target: &PageTarget, content_folder: &Path) -> Result<PageView, PageFault> {
    let folder = chapter_folder(target, content_folder)?;
    let Chapter { index, pieces } = ocr::read_chapter(&folder)?;
    if !(1..=index.page_count).contains(&target.page) {
        return Err(PageFault::NoSuchPage {
            page_count: index.page_count,
        });
    }
    let printed_page =
        PageIndex::read(&folder.join(page_folder_name(target.page)))?.printed_page_number;
    Ok(PageView {
        doc: target.doc,
        page: target.page,
        book: Some(index.book_title),
        chapter: Some(ChapterLabel {
            number: index.chapter_number,
            name: index.chapter_name,
        }),
        page_count: index.page_count,
        printed_page,
        image: picture(&folder, target.page),
        previous_image: (target.page > 1)
            .then(|| picture(&folder, target.page - 1))
            .flatten(),
        next_image: (target.page < index.page_count)
            .then(|| picture(&folder, target.page + 1))
            .flatten(),
        pieces: pieces
            .into_iter()
            .filter(|piece| piece.id.page == target.page)
            .map(piece_view)
            .collect(),
    })
}

/// Where the chapter is on disk. The folder is made canonical, as ingestion does, so a picture has
/// one path whichever way the folder was found, and one place in the cache of decoded pictures.
fn chapter_folder(target: &PageTarget, content_folder: &Path) -> Result<PathBuf, PageFault> {
    let stored = target
        .folder
        .as_deref()
        .filter(|folder| folder.join(CHAPTER_INDEX_FILE).is_file());
    let folder = match stored {
        Some(folder) => folder.to_path_buf(),
        None => {
            let doc = rag_core::DocId::from(target.doc);
            let mut found = chapters_on_disk([(doc, None)], content_folder);
            found
                .remove(&doc)
                .map(|entry| entry.folder)
                .ok_or(PageFault::FolderUnknown)?
        }
    };
    std::fs::canonicalize(&folder).map_err(|source| {
        PageFault::from(ContentError::Read {
            path: folder,
            source,
        })
    })
}

/// The picture of a page, only when the file is there: a chapter written by hand has none.
fn picture(folder: &Path, page: u32) -> Option<ImageRef> {
    existing(folder.join(page_folder_name(page)).join(PAGE_IMAGE_FILE))
}

fn existing(path: PathBuf) -> Option<ImageRef> {
    path.is_file().then_some(ImageRef { path })
}

fn piece_view(piece: ChapterPiece) -> PagePiece {
    let ChapterPiece {
        id,
        content,
        figure_image,
        detail,
        ..
    } = piece;
    let plain = PagePiece {
        number: id.number,
        kind: PieceKind::Text,
        label: None,
        name: None,
        caption: None,
        text: content,
        image: None,
        cut: None,
    };
    match detail {
        PieceDetail::Heading {
            rank,
            printed_number,
        } => PagePiece {
            kind: PieceKind::Heading { rank },
            label: printed_number,
            ..plain
        },
        PieceDetail::Text { .. } => plain,
        PieceDetail::Formula { label, name, .. } => PagePiece {
            kind: PieceKind::Formula,
            label,
            name,
            ..plain
        },
        PieceDetail::Figure {
            label,
            caption,
            image,
            ..
        } => PagePiece {
            kind: PieceKind::Figure,
            label,
            caption,
            image: figure_image.and_then(|picture| existing(picture.path)),
            cut: image
                .filter(|saved| saved.shows == ImageShows::Figure)
                .and_then(|saved| saved.cut)
                .and_then(page_box),
            ..plain
        },
        PieceDetail::Table { label, caption, .. } => PagePiece {
            kind: PieceKind::Table,
            label,
            caption,
            ..plain
        },
        PieceDetail::Footnote { marker, .. } => PagePiece {
            kind: PieceKind::Footnote,
            label: marker,
            ..plain
        },
    }
}

/// The rectangle, only when it can be drawn on the page: no highlight beats a wrong one.
fn page_box(saved: ocr::PageBox) -> Option<PageBox> {
    let side = |value: i32| u16::try_from(value).ok().filter(|side| *side <= 1000);
    let page_box = PageBox {
        left: side(saved.left)?,
        top: side(saved.top)?,
        right: side(saved.right)?,
        bottom: side(saved.bottom)?,
    };
    (page_box.left < page_box.right && page_box.top < page_box.bottom).then_some(page_box)
}
