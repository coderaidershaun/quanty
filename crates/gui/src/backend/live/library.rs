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

/// The stored documents, each with its chapter on disk. Of the stores only the graph is read, so
/// a vector store that is down does not stop the list.
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

/// Groups the stored documents into books. A book is the one the document was stored with, not
/// the one its chapter on disk names, because an ask that is filtered by a book matches the
/// stored book. Books come by title with capitals ignored, and the documents without a book
/// last. Inside a book the chapters come by number, and the documents with no chapter on disk
/// after them.
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use graph::{DocumentNode, ItemsByKind};
    use ocr::ChapterIndex;
    use ocr::content::FORMAT_VERSION;
    use rag_core::{DocumentLabels, Tag};

    use super::*;

    fn id_of(title: &str) -> rag_core::DocId {
        rag_core::DocId::from_source_sha256(title)
    }

    fn record(title: &str, book: Option<&str>) -> DocumentRecord {
        DocumentRecord {
            node: DocumentNode {
                id: id_of(title),
                title: title.to_owned(),
                labels: DocumentLabels {
                    book: book.map(str::to_owned),
                    ..DocumentLabels::default()
                },
            },
            ingested_items: Some(1),
            chapter_folder: None,
            items: ItemsByKind::default(),
        }
    }

    fn chapter(number: u32, name: &str) -> ChapterEntry {
        ChapterEntry {
            folder: PathBuf::from(format!("/content/book/chapter-{number}")),
            index: ChapterIndex {
                format_version: FORMAT_VERSION,
                book_title: "A Book On Disk".to_owned(),
                chapter_number: number,
                chapter_name: name.to_owned(),
                source_file: format!("chapter-{number}.pdf"),
                source_sha256: format!("hash-{number}"),
                page_count: 7,
                finished: true,
            },
        }
    }

    #[test]
    fn books_come_by_title_and_chapters_by_number() {
        // The books are given out of order, and "banking" comes before "Options" only when
        // capitals are ignored.
        let stored = [
            ("Options chapter 10", Some("Options"), Some(10)),
            ("Stored before labels", None, Some(1)),
            ("Banking chapter 3", Some("banking"), Some(3)),
            ("Options chapter 2", Some("Options"), Some(2)),
            ("Options with no folder", Some("Options"), None),
            ("Banking chapter 1", Some("banking"), Some(1)),
        ];
        let records = stored
            .iter()
            .map(|(title, book, _)| record(title, *book))
            .collect();
        let found = stored
            .iter()
            .filter_map(|(title, _, number)| {
                number.map(|number| (id_of(title), chapter(number, "A chapter")))
            })
            .collect();

        let catalogue = catalogue_from(records, found);

        let shown: Vec<(Option<&str>, Vec<&str>)> = catalogue
            .books
            .iter()
            .map(|book| {
                let titles = book.chapters.iter().map(|d| d.title.as_str()).collect();
                (book.title.as_deref(), titles)
            })
            .collect();
        let expected = vec![
            (
                Some("banking"),
                vec!["Banking chapter 1", "Banking chapter 3"],
            ),
            (
                Some("Options"),
                vec![
                    "Options chapter 2",
                    "Options chapter 10",
                    "Options with no folder",
                ],
            ),
            (None, vec!["Stored before labels"]),
        ];
        assert_eq!(shown, expected);
    }

    #[test]
    fn a_document_with_no_folder_keeps_its_gaps() {
        let mut kept = record("Kept", Some("Options"));
        kept.items.chunks = 5;
        // Its folder was stored, but nothing on disk names this document any more, and its
        // ingest stopped part of the way.
        let mut stopped = record("Stopped", Some("Options"));
        stopped.chapter_folder = Some(PathBuf::from("/moved/away"));
        stopped.ingested_items = None;
        stopped.items = ItemsByKind {
            chunks: 3,
            formulas: 2,
            figures: 1,
            tables: 4,
        };
        let mut picture = record("A lone picture", None);
        picture.node.labels.author = Some("Ada Quant".to_owned());
        picture.node.labels.tags = ["Options", " greeks "]
            .map(|tag| tag.parse::<Tag>().expect("a tag should parse"))
            .into();
        picture.items.figures = 1;
        let found = [(id_of("Kept"), chapter(2, "Smiles"))].into();

        let catalogue = catalogue_from(vec![picture, stopped, kept], found);

        let expected = Catalogue {
            books: vec![
                Book {
                    title: Some("Options".to_owned()),
                    chapters: vec![
                        Document {
                            id: DocId::from(id_of("Kept")),
                            title: "Kept".to_owned(),
                            chapter: Some(ChapterLabel {
                                number: 2,
                                name: "Smiles".to_owned(),
                            }),
                            author: None,
                            tags: Vec::new(),
                            pages: Some(7),
                            items: ItemCounts {
                                chunks: 5,
                                ..ItemCounts::default()
                            },
                            ingested_items: Some(1),
                            folder: Some(PathBuf::from("/content/book/chapter-2")),
                        },
                        Document {
                            id: DocId::from(id_of("Stopped")),
                            title: "Stopped".to_owned(),
                            chapter: None,
                            author: None,
                            tags: Vec::new(),
                            pages: None,
                            items: ItemCounts {
                                chunks: 3,
                                formulas: 2,
                                figures: 1,
                                tables: 4,
                            },
                            ingested_items: None,
                            folder: None,
                        },
                    ],
                },
                Book {
                    title: None,
                    chapters: vec![Document {
                        id: DocId::from(id_of("A lone picture")),
                        title: "A lone picture".to_owned(),
                        chapter: None,
                        author: Some("Ada Quant".to_owned()),
                        tags: vec!["greeks".to_owned(), "options".to_owned()],
                        pages: None,
                        items: ItemCounts {
                            figures: 1,
                            ..ItemCounts::default()
                        },
                        ingested_items: Some(1),
                        folder: None,
                    }],
                },
            ],
        };
        assert_eq!(catalogue, expected);
    }
}
