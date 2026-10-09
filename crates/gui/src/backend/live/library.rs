//! Lists the stored documents grouped into media, with the media saved before any document,
//! saves a new media, changes the labels of a media and the own tags of a document. Deleting a
//! document is not built yet.

use std::collections::HashMap;

use graph::{DocumentRecord, GraphStore, MediaNode};
use ocr::ChapterEntry;
use ocr::content::media_folder_name;
use rag_ingestion::{MediaChange, TagChange, relabel_document_tags, relabel_media};

use super::chapters::chapters_on_disk;
use super::{LiveContext, Services};
use crate::backend::Reply;
use crate::contract::{
    Catalogue, DocId, Document, DocumentName, DocumentTagsEdit, Event, Failure, ItemCounts, Media,
    MediaEdit, NewMedia, RequestId, is_same_name,
};

/// Sends exactly one catalogue, also when it cannot be read, so the window never waits for one.
pub(super) async fn load_catalogue<S: Services>(
    cx: &LiveContext<S>,
    request: RequestId,
    reply: &Reply,
) {
    let result = read_catalogue(cx).await;
    reply.send(Event::Catalogue { request, result });
}

/// Of the stores only the graph is read, so a vector store that is down does not stop the list.
async fn read_catalogue<S: Services>(cx: &LiveContext<S>) -> Result<Catalogue, Failure> {
    let graph = cx.graph().await?;
    let records = graph
        .document_records()
        .await
        .map_err(|error| cx.failure(error))?;
    let saved = graph.media().await.map_err(|error| cx.failure(error))?;
    let found = chapters_on_disk(
        records
            .iter()
            .map(|record| (record.node.id, record.chapter_folder.as_deref())),
        &cx.config().content_folder,
    );
    Ok(catalogue_from(records, found, saved))
}

/// Sends exactly one answer, also when the tags cannot be written.
pub(super) async fn set_document_tags<S: Services>(
    cx: &LiveContext<S>,
    request: RequestId,
    edit: &DocumentTagsEdit,
    reply: &Reply,
) {
    let result = write_document_tags(cx, edit).await;
    reply.send(Event::DocumentTagsSaved {
        request,
        doc: edit.doc,
        result,
    });
}

/// It makes no embedder and asks no model: only the tags are written, in both stores.
async fn write_document_tags<S: Services>(
    cx: &LiveContext<S>,
    edit: &DocumentTagsEdit,
) -> Result<(), Failure> {
    let stores = cx.stores().await?;
    relabel_document_tags(edit.doc.into(), &TagChange::from(edit), &stores)
        .await
        .map(|_| ())
        .map_err(|error| cx.failure(error))
}

pub(super) async fn delete<S: Services>(
    _cx: &LiveContext<S>,
    request: RequestId,
    doc: DocId,
    reply: &Reply,
) {
    reply.send(Event::Deleted {
        request,
        doc,
        result: Err(Failure::not_built("deleting a document")),
    });
}

/// Sends exactly one answer, also when the media is refused or cannot be stored.
pub(super) async fn save_media<S: Services>(
    cx: &LiveContext<S>,
    request: RequestId,
    media: &NewMedia,
    reply: &Reply,
) {
    let result = store_media(cx, media).await;
    reply.send(Event::MediaSaved { request, result });
}

/// The title must be one that a folder can be named after, and the library must not have it yet,
/// whether as a saved media or as a label on stored documents.
// SMELL: the check and the write are two steps, so another program that saves a media between
// them can store a second media whose title differs only in capitals. The store compares titles
// as they are given, so only a change to how it writes a media can close the gap.
async fn store_media<S: Services>(cx: &LiveContext<S>, media: &NewMedia) -> Result<(), Failure> {
    media_folder_name(&media.title).map_err(|error| cx.failure(error))?;
    if let Some(stored) = read_catalogue(cx).await?.stored_title(&media.title) {
        return Err(Failure::media_exists(stored));
    }
    let graph = cx.graph().await?;
    graph
        .add_media(&MediaNode::from(media))
        .await
        .map_err(|error| cx.failure(error))
}

/// Sends exactly one answer, also when the media is unknown or a store fails.
pub(super) async fn edit_media<S: Services>(
    cx: &LiveContext<S>,
    request: RequestId,
    edit: &MediaEdit,
    reply: &Reply,
) {
    let result = write_media(cx, edit).await;
    reply.send(Event::MediaEdited { request, result });
}

/// It makes no embedder and asks no model: the media and every document of it are relabelled, in
/// both stores.
async fn write_media<S: Services>(cx: &LiveContext<S>, edit: &MediaEdit) -> Result<(), Failure> {
    let stores = cx.stores().await?;
    relabel_media(&edit.title, &MediaChange::from(edit), &stores)
        .await
        .map(|_| ())
        .map_err(|error| cx.failure(error))
}

/// A document as the catalogue lists it, with the media it was stored with: an ask that is
/// filtered by a media matches the stored one, not the one its folder on disk names.
struct Listed {
    media: Option<String>,
    category: Option<rag_core::Category>,
    document: Document,
}

/// Each stored media holds the documents whose media has its title, whatever the capitals, and a
/// media saved before its first document holds none. Documents whose media has no node are grouped
/// by title and take the labels of the media from their first document, since every document
/// carries a copy. Documents of no media are one group with no title.
fn catalogue_from(
    records: Vec<DocumentRecord>,
    mut found: HashMap<rag_core::DocId, ChapterEntry>,
    saved: Vec<MediaNode>,
) -> Catalogue {
    let mut left: Vec<Listed> = records
        .into_iter()
        .map(|record| {
            let chapter = found.remove(&record.node.id);
            document_of(record, chapter)
        })
        .collect();
    let mut media: Vec<Media> = Vec::new();
    for node in saved {
        let (of_node, others): (Vec<Listed>, Vec<Listed>) = left.into_iter().partition(|listed| {
            listed
                .media
                .as_deref()
                .is_some_and(|title| is_same_name(title, &node.title))
        });
        left = others;
        media.push(Media {
            title: Some(node.title),
            category: node.labels.category.into(),
            authors: node.labels.authors,
            tags: node.labels.tags.iter().map(ToString::to_string).collect(),
            documents: of_node.into_iter().map(|listed| listed.document).collect(),
        });
    }
    for listed in left {
        match media.iter_mut().find(|group| group.title == listed.media) {
            Some(group) => group.documents.push(listed.document),
            None => media.push(Media {
                title: listed.media,
                category: listed.category.map(Into::into).unwrap_or_default(),
                authors: listed.document.authors.clone(),
                tags: listed.document.media_tags.clone(),
                documents: vec![listed.document],
            }),
        }
    }
    for group in &mut media {
        group.documents.sort_by_cached_key(reading_order);
    }
    let mut catalogue = Catalogue { media };
    catalogue.sort_media();
    catalogue
}

/// Chapters come first, by number, then the other documents by title.
fn reading_order(document: &Document) -> (bool, Option<u32>, String, DocId) {
    let number = document.chapter.as_ref().map(|chapter| chapter.number);
    (
        number.is_none(),
        number,
        document.title.clone(),
        document.id,
    )
}

/// What the document on disk gives is left empty when it is not there, never made up. A
/// document with a title of its own has no chapter.
fn document_of(record: DocumentRecord, entry: Option<ChapterEntry>) -> Listed {
    let DocumentRecord {
        node,
        ingested_items,
        items,
        ..
    } = record;
    let labels = node.labels;
    let (chapter, pages, folder) = match entry {
        Some(ChapterEntry { folder, index }) => {
            let chapter = DocumentName::from(index.name).chapter_label();
            (chapter, Some(index.page_count), Some(folder))
        }
        None => (None, None, None),
    };
    let document = Document {
        id: node.id.into(),
        title: node.title,
        chapter,
        authors: labels.authors,
        media_tags: labels.media_tags.iter().map(ToString::to_string).collect(),
        tags: labels.tags.iter().map(ToString::to_string).collect(),
        pages,
        items: ItemCounts {
            chunks: items.chunks,
            formulas: items.formulas,
            figures: items.figures,
            tables: items.tables,
        },
        ingested_items,
        folder,
    };
    Listed {
        media: labels.media,
        category: labels.category,
        document,
    }
}
