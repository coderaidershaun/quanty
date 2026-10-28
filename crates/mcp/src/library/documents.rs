//! The `list_documents` tool: the documents of the graph that fit the labels asked for, with their
//! labels and, when one is converted under the content folder, its chapter.

use std::collections::HashMap;

use graph::{DocumentRecord, FalkorGraph, ItemsByKind};
use ocr::{Catalogue, ChapterEntry};
use rag_core::{Category, Config, DocId, DocumentLabels, LabelFilter};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::LibraryError;
use super::chapter::{chapter_of, document_of, unreadable_chapters};
use super::discovery::Discovery;
use crate::args::{BudgetError, Position, Scope, non_blank, parse_optional, parse_tags};

/// How many documents a page holds unless the caller asks for another number.
const DEFAULT_LIMIT: u32 = 100;

/// The most documents a page may hold.
const LARGEST_LIMIT: u32 = 500;

/// What an agent gives `list_documents`. Every part is optional.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct ListDocumentsArgs {
    /// Only documents of this media, whatever the capitals.
    media: Option<String>,
    /// Only documents of this category: `book`, `paper` or `other`.
    category: Option<String>,
    /// Only documents by this author, whatever the capitals.
    author: Option<String>,
    /// Only documents that have every one of these tags, each as a tag of the media or of the
    /// document.
    tags: Option<Vec<String>>,
    /// `full` (default), `summary` or `names`.
    detail: Option<DocumentsDetail>,
    /// From a result that has one, with the same filters.
    cursor: Option<String>,
    /// How many documents at most, from 1 to 500. 100 when left out.
    limit: Option<u32>,
}

/// How much of each document `list_documents` gives.
#[derive(Deserialize, JsonSchema, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DocumentsDetail {
    /// Labels, chapter, pages, items and whether it was ingested whole.
    #[default]
    Full,
    /// The id, title, media, category, pages and whether it was ingested whole.
    Summary,
    /// The id and the title.
    Names,
}

/// The stored documents that fit, one page of them.
#[derive(Serialize, JsonSchema)]
pub(crate) struct Documents {
    /// Sorted by title.
    documents: Vec<DocumentView>,
    /// How many documents fit the filters, over all pages.
    total: usize,
    /// Give it back with the same filters for the next page. Only when more documents fit.
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<String>,
    /// One line for each converted chapter under the content folder that cannot be read, with
    /// the file that is at fault. The document of such a chapter shows no chapter above, and
    /// `read_page` cannot read it. Empty when every chapter can be read.
    unreadable_chapters: Vec<String>,
    /// Only with `detail: names`.
    #[serde(flatten)]
    discovery: Option<Discovery>,
}

#[derive(Serialize, JsonSchema)]
struct DocumentView {
    /// Give it to `read_page`, or as `document` to `search` and `answer`.
    document_id: String,
    /// The whole stored title of the document: the media and its chapter or part, or a title of
    /// its own. Give it as `document` to `search` and `answer` to ask this document alone.
    title: String,
    /// The title of the media the document belongs to. A picture that stands alone has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    media: Option<String>,
    /// `book`, `paper` or `other`: the category of the media.
    #[serde(skip_serializing_if = "Option::is_none")]
    category: Option<String>,
    /// The authors of the media.
    #[serde(skip_serializing_if = "Option::is_none")]
    authors: Option<Vec<String>>,
    /// The tags of the media.
    #[serde(skip_serializing_if = "Option::is_none")]
    media_tags: Option<Vec<String>>,
    /// The tags of this document only.
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Vec<String>>,
    /// The chapter number, when the document is a converted book chapter under the content
    /// folder.
    #[serde(skip_serializing_if = "Option::is_none")]
    chapter_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    chapter_name: Option<String>,
    /// How many pages the document has, when it is converted under the content folder.
    #[serde(skip_serializing_if = "Option::is_none")]
    pages: Option<u32>,
    /// False when an ingest stopped or skipped an item, or the document was stored before quanty
    /// kept this mark: a search may miss part of it. Send its PDF again with `ingest_pdf` to
    /// finish it.
    #[serde(skip_serializing_if = "Option::is_none")]
    ingested_whole: Option<bool>,
    /// How many items of each kind the stores hold for the document.
    #[serde(skip_serializing_if = "Option::is_none")]
    items: Option<StoredItems>,
}

impl DocumentView {
    /// The same document with only the parts that `detail` gives.
    fn shown_as(self, detail: DocumentsDetail) -> DocumentView {
        if detail == DocumentsDetail::Full {
            return self;
        }
        let summary = DocumentView {
            authors: None,
            media_tags: None,
            tags: None,
            chapter_number: None,
            chapter_name: None,
            items: None,
            ..self
        };
        if detail == DocumentsDetail::Summary {
            return summary;
        }
        DocumentView {
            media: None,
            category: None,
            pages: None,
            ingested_whole: None,
            ..summary
        }
    }
}

#[derive(Serialize, JsonSchema)]
struct StoredItems {
    chunk: u64,
    formula: u64,
    figure: u64,
    table: u64,
    code: u64,
}

impl From<ItemsByKind> for StoredItems {
    fn from(items: ItemsByKind) -> StoredItems {
        StoredItems {
            chunk: items.chunks,
            formula: items.formulas,
            figure: items.figures,
            table: items.tables,
            code: items.code,
        }
    }
}

/// The documents of the graph that fit the filters, one page of them, each with its labels and its
/// chapter when that is under the content folder. A chapter that cannot be read is named, and does
/// not hide the others.
///
/// # Errors
/// A bad argument, the graph that is not ready, or a content folder that cannot be listed.
pub(crate) async fn list_documents(
    config: &Config,
    args: ListDocumentsArgs,
) -> Result<Documents, LibraryError> {
    let detail = args.detail.unwrap_or_default();
    let limit = checked_limit(args.limit)?;
    let wanted = LabelFilter {
        document: None,
        media: non_blank(args.media),
        author: non_blank(args.author),
        category: parse_optional::<Category>(args.category).map_err(LibraryError::Category)?,
        tags: parse_tags(args.tags).map_err(LibraryError::FilterTag)?,
    };
    let scope = Scope::of("list_documents").with_fingerprint(&format!("{wanted:?}"));
    let start = match non_blank(args.cursor) {
        Some(cursor) => scope.resume(&cursor)?.unit,
        None => 0,
    };
    let graph = FalkorGraph::connect(config).await?;
    let records = graph.document_records().await?;
    let catalogue = Catalogue::read(&config.content_folder)?;
    let chapters: HashMap<DocId, &ChapterEntry> = catalogue
        .chapters
        .iter()
        .map(|entry| (document_of(entry), entry))
        .collect();
    let mut documents: Vec<DocumentView> = records
        .into_iter()
        .filter(|record| wanted.fits(record.node.id, &record.node.title, &record.node.labels))
        .map(|record| document_view(record, &chapters))
        .collect();
    documents.sort_by(|a, b| a.title.cmp(&b.title));
    let total = documents.len();
    if start > total {
        return Err(BudgetError::ForeignCursor.into());
    }
    let end = (start + limit).min(total);
    let cursor = (end < total).then(|| {
        scope.cursor(Position {
            unit: end,
            offset: 0,
        })
    });
    let page: Vec<DocumentView> = documents
        .drain(start..end)
        .map(|document| document.shown_as(detail))
        .collect();
    let discovery = (detail == DocumentsDetail::Names).then(|| {
        page.iter().fold(Discovery::new(), |discovery, document| {
            let arguments = json!({ "document_id": document.document_id });
            discovery.discovering(&document.title, "document_outline", arguments)
        })
    });
    Ok(Documents {
        documents: page,
        total,
        cursor,
        unreadable_chapters: unreadable_chapters(&catalogue),
        discovery,
    })
}

/// How many documents a page holds for a `limit` argument.
fn checked_limit(limit: Option<u32>) -> Result<usize, LibraryError> {
    match limit {
        None => Ok(DEFAULT_LIMIT as usize),
        Some(given) if (1..=LARGEST_LIMIT).contains(&given) => Ok(given as usize),
        Some(given) => Err(LibraryError::LimitOutOfRange {
            given,
            most: LARGEST_LIMIT,
        }),
    }
}

fn document_view(record: DocumentRecord, chapters: &HashMap<DocId, &ChapterEntry>) -> DocumentView {
    let DocumentRecord {
        node,
        ingested_items,
        items,
        ..
    } = record;
    let index = chapters.get(&node.id).map(|entry| &entry.index);
    let chapter = index.and_then(|index| chapter_of(&index.name));
    let DocumentLabels {
        media,
        category,
        authors,
        media_tags,
        tags,
    } = node.labels;
    DocumentView {
        document_id: node.id.to_string(),
        title: node.title,
        media,
        category: category.map(|category| category.as_str().to_owned()),
        authors: Some(authors),
        media_tags: Some(media_tags.iter().map(ToString::to_string).collect()),
        tags: Some(tags.iter().map(ToString::to_string).collect()),
        chapter_number: chapter.map(|(number, _)| number),
        chapter_name: chapter.map(|(_, name)| name.to_owned()),
        pages: index.map(|index| index.page_count),
        ingested_whole: Some(ingested_items.is_some()),
        items: Some(items.into()),
    }
}
