//! Lists the stored documents, each joined with its chapter on disk and grouped into books, with
//! the books a person saved before any chapter, and saves a new book. Changing the labels of a
//! document and deleting one are not built yet.

use std::collections::HashMap;

use graph::{BookNode, DocumentRecord, GraphStore};
use ocr::ChapterEntry;
use ocr::content::book_folder_name;

use super::chapters::chapters_on_disk;
use super::{LiveContext, Services};
use crate::backend::Reply;
use crate::contract::{
    Book, Catalogue, ChapterLabel, DocId, Document, Event, Failure, ItemCounts, LabelEdit, NewBook,
    RequestId, is_same_title,
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
    let saved = graph.books().await.map_err(|error| cx.failure(error))?;
    let found = chapters_on_disk(
        records
            .iter()
            .map(|record| (record.node.id, record.chapter_folder.as_deref())),
        &cx.config().content_folder,
    );
    Ok(catalogue_from(records, found, saved))
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

/// Sends exactly one answer, also when the book is refused or cannot be stored.
pub async fn save_book<S: Services>(
    cx: &LiveContext<S>,
    request: RequestId,
    book: &NewBook,
    reply: &Reply,
) {
    let result = store_book(cx, book).await;
    reply.send(Event::BookSaved { request, result });
}

/// The title must be one that a chapter folder can be named after, and the library must not have
/// it yet, whether as a saved book or as a label on stored documents.
// SMELL: the check and the write are two steps, so another program that saves a book between
// them can store a second book whose title differs only in capitals. The store itself treats
// only the exact same title as the same book.
async fn store_book<S: Services>(cx: &LiveContext<S>, book: &NewBook) -> Result<(), Failure> {
    book_folder_name(&book.title).map_err(|error| cx.failure(error))?;
    if let Some(stored) = read_catalogue(cx).await?.stored_title(&book.title) {
        return Err(Failure::book_exists(stored));
    }
    let graph = cx.graph().await?;
    graph
        .add_book(&BookNode::from(book))
        .await
        .map_err(|error| cx.failure(error))
}

/// A book is the one the document was stored with, not the one its chapter on disk names, because
/// an ask that is filtered by a book matches the stored book.
fn catalogue_from(
    records: Vec<DocumentRecord>,
    mut found: HashMap<rag_core::DocId, ChapterEntry>,
    saved: Vec<BookNode>,
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
                author: None,
                tags: Vec::new(),
                chapters: vec![document],
            }),
        }
    }
    for saved in saved {
        join_saved_book(&mut books, saved);
    }
    let mut catalogue = Catalogue { books };
    catalogue.sort_books();
    catalogue
}

/// A saved book gives its author and tags to the books of the documents that have its title, and
/// is a book with no chapter when none has.
fn join_saved_book(books: &mut Vec<Book>, saved: BookNode) {
    let author = saved.author;
    let tags: Vec<String> = saved.tags.iter().map(ToString::to_string).collect();
    let mut has_documents = false;
    for book in books.iter_mut() {
        if book
            .title
            .as_deref()
            .is_some_and(|title| is_same_title(title, &saved.title))
        {
            book.author = author.clone();
            book.tags = tags.clone();
            has_documents = true;
        }
    }
    if !has_documents {
        books.push(Book {
            title: Some(saved.title),
            author,
            tags,
            chapters: Vec::new(),
        });
    }
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
