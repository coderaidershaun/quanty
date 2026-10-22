//! The `list_documents` tool: every document of the graph with its labels and, when it is
//! converted under the content folder, its chapter.

use std::collections::HashMap;

use graph::{DocumentRecord, FalkorGraph, ItemsByKind};
use ocr::{Catalogue, ChapterEntry};
use rag_core::{Config, DocId, DocumentLabels};
use schemars::JsonSchema;
use serde::Serialize;

use super::LibraryError;
use super::chapter::{chapter_of, document_of, unreadable_chapters};

/// Every stored document.
#[derive(Serialize, JsonSchema)]
pub(crate) struct Documents {
    /// Sorted by title.
    documents: Vec<DocumentView>,
    /// One line for each converted chapter under the content folder that cannot be read, with
    /// the file that is at fault. The document of such a chapter shows no chapter above, and
    /// `read_page` cannot read it. Empty when every chapter can be read.
    unreadable_chapters: Vec<String>,
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
    authors: Vec<String>,
    /// The tags of the media.
    media_tags: Vec<String>,
    /// The tags of this document only.
    tags: Vec<String>,
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
    ingested_whole: bool,
    /// How many items of each kind the stores hold for the document.
    items: StoredItems,
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

/// Every document of the graph with its labels, and its chapter when that is under the content
/// folder. A chapter that cannot be read is named, and does not hide the others.
///
/// # Errors
/// The graph that is not ready, or a content folder that cannot be listed.
pub(crate) async fn list_documents(config: &Config) -> Result<Documents, LibraryError> {
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
        .map(|record| {
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
                authors,
                media_tags: media_tags.iter().map(ToString::to_string).collect(),
                tags: tags.iter().map(ToString::to_string).collect(),
                chapter_number: chapter.map(|(number, _)| number),
                chapter_name: chapter.map(|(_, name)| name.to_owned()),
                pages: index.map(|index| index.page_count),
                ingested_whole: ingested_items.is_some(),
                items: items.into(),
            }
        })
        .collect();
    documents.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(Documents {
        documents,
        unreadable_chapters: unreadable_chapters(&catalogue),
    })
}
