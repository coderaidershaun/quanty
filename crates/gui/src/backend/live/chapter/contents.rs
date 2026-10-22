//! Reads every formula, figure, table and code listing of a saved chapter from disk, so the
//! Library shows them while the stores are down, and the concepts of the whole document from the
//! graph.

use std::path::{Path, PathBuf};

use ocr::ReadChapterError;

use super::folder::{FolderFault, chapter_folder};
use super::source::piece_view;
use crate::backend::Reply;
use crate::backend::live::{LiveContext, Services};
use crate::contract::{
    ChapterContents, ChapterItem, DocId, Event, Failure, PageConcept, PagePiece, RequestId,
};

/// Sends the items, then the concepts: always both, always in this order. The window takes the
/// concepts as the end of the load, so it would wait for ever without them.
pub(in crate::backend::live) async fn load_chapter_contents<S: Services>(
    cx: &LiveContext<S>,
    request: RequestId,
    doc: DocId,
    folder: Option<PathBuf>,
    reply: &Reply,
) {
    let items = read_items(cx, doc, folder).await;
    reply.send(Event::ChapterContents {
        request,
        result: items,
    });
    let concepts = document_concepts(cx, doc).await;
    reply.send(Event::ChapterConcepts {
        request,
        result: concepts,
    });
}

#[derive(Debug)]
enum ItemsFault {
    Folder(FolderFault),
    Read(ReadChapterError),
}

async fn read_items<S: Services>(
    cx: &LiveContext<S>,
    doc: DocId,
    folder: Option<PathBuf>,
) -> Result<ChapterContents, Failure> {
    let content_folder = cx.config().content_folder.clone();
    let read = tokio::task::spawn_blocking(move || {
        items_from_disk(doc, folder.as_deref(), &content_folder)
    })
    .await;
    match read {
        Ok(Ok(contents)) => Ok(contents),
        Ok(Err(ItemsFault::Folder(fault))) => Err(fault.failure(cx, doc.into())),
        Ok(Err(ItemsFault::Read(error))) => Err(cx.failure(error)),
        Err(error) => Err(Failure::internal(format!(
            "the chapter reader stopped: {error}"
        ))),
    }
}

async fn document_concepts<S: Services>(
    cx: &LiveContext<S>,
    doc: DocId,
) -> Result<Vec<PageConcept>, Failure> {
    let graph = cx.graph().await?;
    let concepts = graph
        .concepts_in_document(doc.into())
        .await
        .map_err(|error| cx.failure(error))?;
    Ok(concepts.into_iter().map(PageConcept::from).collect())
}

fn items_from_disk(
    doc: DocId,
    given: Option<&Path>,
    content_folder: &Path,
) -> Result<ChapterContents, ItemsFault> {
    let folder = chapter_folder(doc.into(), given, content_folder).map_err(ItemsFault::Folder)?;
    let chapter = ocr::read_chapter(&folder).map_err(ItemsFault::Read)?;
    Ok(ChapterContents {
        doc,
        page_count: chapter.index.page_count,
        items: {
            let source = chapter.artifact_source();
            let saved = ocr::content::ChapterArtifacts::read(&folder)
                .ok()
                .flatten()
                .filter(|saved| saved.matches_source(&source));
            chapter
                .items()
                .iter()
                .map(|item| {
                    let mut view = item_view(item);
                    let id = format!(
                        "{}:{}:{}",
                        chapter.index.source_sha256, item.first.id.page, item.first.id.number
                    );
                    view.metadata = saved
                        .as_ref()
                        .and_then(|saved| saved.text(&id))
                        .map(|text| crate::contract::ArtifactMetadata {
                            title: text.title.clone(),
                            description: text.description.clone(),
                        });
                    view
                })
                .collect()
        },
    })
}

fn item_view(item: &ocr::ChapterItem<'_>) -> ChapterItem {
    let piece = PagePiece {
        text: item.content(),
        language: item.language().map(str::to_owned),
        ..piece_view(item.first.clone())
    };
    ChapterItem {
        page: item.first.id.page,
        printed_page: item.first.printed_page_number.clone(),
        last_page: item.last_page(),
        last_printed_page: item.last_printed_page().map(str::to_owned),
        piece,
        metadata: None,
    }
}
