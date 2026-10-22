//! Reads one page of a saved chapter from disk, so the page shows while the stores are down, and
//! the concepts of that page from the graph.

use crate::live::convert::IntoView;

use std::path::{Path, PathBuf};

use ocr::content::{PAGE_IMAGE_FILE, PageIndex, own_title, page_folder_name};
use ocr::{Chapter, ChapterPiece, ContentError, ImageShows, PieceDetail, ReadChapterError};

use super::folder::{FolderFault, chapter_folder};
use crate::Reply;
use crate::live::{LiveContext, Services};
use contract::{
    DocId, Event, Failure, ImageRef, PageBox, PageConcept, PagePiece, PageView, PieceKind,
    RequestId,
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

#[derive(Debug)]
enum PageFault {
    Read(ReadChapterError),
    Folder(FolderFault),
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
            PageFault::Folder(fault) => fault.failure(cx, target.doc.into_view()),
            PageFault::NoSuchPage { page_count } => Failure {
                detail: format!(
                    "page {} is not in document {}, which has {page_count} pages",
                    target.page, target.doc.0
                ),
                ..Failure::no_such_page(target.page, page_count)
            },
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
        .concepts_on_page(target.doc.into_view(), target.page)
        .await
        .map_err(|error| cx.failure(error))?;
    Ok(concepts.into_iter().map(IntoView::into_view).collect())
}

fn page_from_disk(target: &PageTarget, content_folder: &Path) -> Result<PageView, PageFault> {
    let folder = chapter_folder(
        target.doc.into_view(),
        target.folder.as_deref(),
        content_folder,
    )
    .map_err(PageFault::Folder)?;
    let Chapter { index, pieces } = ocr::read_chapter(&folder)?;
    if !(1..=index.page_count).contains(&target.page) {
        return Err(PageFault::NoSuchPage {
            page_count: index.page_count,
        });
    }
    let printed_page =
        PageIndex::read(&folder.join(page_folder_name(target.page)))?.printed_page_number;
    let name = index.name.into_view();
    Ok(PageView {
        doc: target.doc,
        page: target.page,
        chapter: name.chapter_label(),
        document_title: name
            .title()
            .map(|title| own_title(&index.media_title, title).to_owned()),
        media: Some(index.media_title),
        page_count: index.page_count,
        printed_page,
        image: picture(&folder, target.page, content_folder),
        previous_image: (target.page > 1)
            .then(|| picture(&folder, target.page - 1, content_folder))
            .flatten(),
        next_image: (target.page < index.page_count)
            .then(|| picture(&folder, target.page + 1, content_folder))
            .flatten(),
        pieces: pieces
            .into_iter()
            .filter(|piece| piece.id.page == target.page)
            .map(|piece| piece_view(piece, content_folder))
            .collect(),
    })
}

/// The picture of a page, only when the file is there: a chapter written by hand has none.
fn picture(folder: &Path, page: u32, content_folder: &Path) -> Option<ImageRef> {
    existing(
        folder.join(page_folder_name(page)).join(PAGE_IMAGE_FILE),
        content_folder,
    )
}

fn existing(path: PathBuf, content_folder: &Path) -> Option<ImageRef> {
    path.is_file()
        .then(|| crate::content::image_url(&path, content_folder))
        .flatten()
        .map(|url| ImageRef { url })
}

pub(super) fn piece_view(piece: ChapterPiece, content_folder: &Path) -> PagePiece {
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
        language: None,
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
            image: figure_image.and_then(|picture| existing(picture.path, content_folder)),
            cut: image
                .and_then(|saved| match saved.shows {
                    ImageShows::Figure { cut, .. } => cut,
                    ImageShows::WholePage => None,
                })
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
        PieceDetail::Code {
            label,
            caption,
            language,
            ..
        } => PagePiece {
            kind: PieceKind::Code,
            label,
            caption,
            language,
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
