//! Lists the stored documents, each joined with its chapter on disk and grouped into books.
//! Changing the labels of a document and deleting one are not built yet.

use std::collections::HashMap;

use graph::{DocumentRecord, GraphStore};
use ocr::ChapterEntry;

use super::chapters::chapters_on_disk;
use super::{LiveContext, Services};
use crate::backend::Reply;
use crate::contract::{
    Book, Catalogue, ChapterLabel, DocId, Document, Event, Failure, ItemCounts, LabelEdit,
    RequestId,
};

/// Sends exactly one catalogue, also when it cannot be read, so the window never waits for one.
pub async fn load_catalogue<S: Services>(cx: &LiveContext<S>, request: RequestId, reply: &Reply) {
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
    let found = chapters_on_disk(
        records
            .iter()
            .map(|record| (record.node.id, record.chapter_folder.as_deref())),
        &cx.config().content_folder,
    );
    Ok(catalogue_from(records, found))
}

pub async fn set_labels<S: Services>(
    _cx: &LiveContext<S>,
    request: RequestId,
    edit: &LabelEdit,
    reply: &Reply,
) {
    reply.send(Event::LabelsSaved {
        request,
        doc: edit.doc,
        result: Err(Failure::not_built("changing labels")),
    });
}

pub async fn delete<S: Services>(
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

/// A book is the one the document was stored with, not the one its chapter on disk names, because
/// an ask that is filtered by a book matches the stored book.
fn catalogue_from(
    records: Vec<DocumentRecord>,
    mut found: HashMap<rag_core::DocId, ChapterEntry>,
) -> Catalogue {
    let mut documents: Vec<(Option<String>, Document)> = records
        .into_iter()
        .map(|record| {
            let chapter = found.remove(&record.node.id);
            document_of(record, chapter)
        })
        .collect();
    documents.sort_by_cached_key(|(book, document)| {
        let number = document.chapter.as_ref().map(|chapter| chapter.number);
        (
            book.is_none(),
            book.as_deref().map(str::to_lowercase),
            book.clone(),
            number.is_none(),
            number,
            document.title.clone(),
            document.id,
        )
    });

    let mut books: Vec<Book> = Vec::new();
    for (title, document) in documents {
        match books.last_mut() {
            Some(book) if book.title == title => book.chapters.push(document),
            _ => books.push(Book {
                title,
                chapters: vec![document],
            }),
        }
    }
    Catalogue { books }
}

/// The stored book of a document, and the document as the catalogue lists it. What the chapter
/// on disk gives is left empty when there is no chapter, never made up.
fn document_of(
    record: DocumentRecord,
    chapter: Option<ChapterEntry>,
) -> (Option<String>, Document) {
    let DocumentRecord {
        node,
        ingested_items,
        items,
        ..
    } = record;
    let labels = node.labels;
    let (label, pages, folder) = match chapter {
        Some(ChapterEntry { folder, index }) => (
            Some(ChapterLabel {
                number: index.chapter_number,
                name: index.chapter_name,
            }),
            Some(index.page_count),
            Some(folder),
        ),
        None => (None, None, None),
    };
    let document = Document {
        id: node.id.into(),
        title: node.title,
        chapter: label,
        author: labels.author,
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
    (labels.book, document)
}
