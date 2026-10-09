//! The statements that read documents and media, whether a document is ingested whole and where
//! it was ingested from, with the code that runs each one and the row it expects.

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

use falkordb::FalkorValue;
use rag_core::{Category, DocId, DocumentLabels, MediaLabels, Tag};

use super::{FalkorGraph, id_list, id_value, unreadable_reply};
use crate::contents::{DocumentNode, MediaNode};
use crate::store::GraphError;

const DOCUMENT_ROW: &str = "a document (an id, a title, a media or null, a category or null, a \
list of authors or null, a list of media tags or null and a list of tags or null)";
const MEDIA_ROW: &str =
    "a media (a title, a category, a list of authors or null and a list of tags or null)";
const INGESTED_ITEMS_ROW: &str = "a count of items or null (one whole number that is not negative)";
const FOLDER_ROW: &str = "a document with its folder (an id and a path)";

/// What a statement returns for a document `d`, in the order that `document_from_row` reads it.
/// Every statement that reads a document takes its columns from here.
pub(super) const DOCUMENT_COLUMNS: &str =
    "d.id, d.title, d.media, d.category, d.authors, d.media_tags, d.tags";

// SMELL: each question with a label filter, and each change of a media's labels, reads every
// document, because the rule that a document fits lives in Rust on purpose. Documents are few next
// to items, so this stays cheap.
fn documents_statement() -> String {
    format!(
        "\
MATCH (d:Document)
RETURN {DOCUMENT_COLUMNS}
ORDER BY d.id"
    )
}

fn document_statement() -> String {
    format!(
        "\
MATCH (d:Document {{id: $id}})
RETURN {DOCUMENT_COLUMNS}
LIMIT 1"
    )
}

const MEDIA: &str = "\
MATCH (m:Media)
RETURN m.title, m.category, m.authors, m.tags
ORDER BY m.title";

const INGESTED_ITEMS: &str = "\
MATCH (d:Document {id: $id})
RETURN d.ingested_items
LIMIT 1";

const CHAPTER_FOLDERS: &str = "\
MATCH (d:Document)
WHERE d.id IN $documents AND d.chapter_folder IS NOT NULL
RETURN d.id, d.chapter_folder";

pub(super) async fn documents(graph: &FalkorGraph) -> Result<Vec<DocumentNode>, GraphError> {
    let action = "read the documents";
    let reply = graph
        .run(action, &documents_statement(), Vec::new())
        .await?;
    reply
        .data
        .into_values_lossy()
        .map(document_from_row)
        .collect::<Result<_, _>>()
        .map_err(|found| unreadable_reply(graph, action, DOCUMENT_ROW, found))
}

pub(super) async fn document(
    graph: &FalkorGraph,
    id: DocId,
) -> Result<Option<DocumentNode>, GraphError> {
    let action = "read the document";
    let parameters = vec![("id", id_value(id))];
    let reply = graph.run(action, &document_statement(), parameters).await?;
    reply
        .data
        .into_values_lossy()
        .next()
        .map(document_from_row)
        .transpose()
        .map_err(|found| unreadable_reply(graph, action, DOCUMENT_ROW, found))
}

pub(super) async fn media(graph: &FalkorGraph) -> Result<Vec<MediaNode>, GraphError> {
    let action = "read the media";
    let reply = graph.run(action, MEDIA, Vec::new()).await?;
    reply
        .data
        .into_values_lossy()
        .map(media_from_row)
        .collect::<Result<_, _>>()
        .map_err(|found| unreadable_reply(graph, action, MEDIA_ROW, found))
}

pub(super) async fn ingested_items(
    graph: &FalkorGraph,
    document: DocId,
) -> Result<Option<u64>, GraphError> {
    let action = "read how many items the document was ingested with";
    let parameters = vec![("id", id_value(document))];
    let reply = graph.run(action, INGESTED_ITEMS, parameters).await?;
    match reply.data.into_values_lossy().next() {
        None => Ok(None),
        Some(row) => ingested_items_from_row(row)
            .map_err(|found| unreadable_reply(graph, action, INGESTED_ITEMS_ROW, found)),
    }
}

pub(super) async fn chapter_folders(
    graph: &FalkorGraph,
    documents: &[DocId],
) -> Result<HashMap<DocId, PathBuf>, GraphError> {
    if documents.is_empty() {
        return Ok(HashMap::new());
    }
    let action = "read the folders the documents were ingested from";
    let parameters = vec![("documents", id_list(documents))];
    let reply = graph.run(action, CHAPTER_FOLDERS, parameters).await?;
    reply
        .data
        .into_values_lossy()
        .map(folder_from_row)
        .collect::<Result<_, _>>()
        .map_err(|found| unreadable_reply(graph, action, FOLDER_ROW, found))
}

pub(super) fn document_from_row(row: Vec<FalkorValue>) -> Result<DocumentNode, String> {
    let row = <[FalkorValue; 7]>::try_from(row).map_err(|row| format!("{row:?}"))?;
    let [
        FalkorValue::String(id),
        FalkorValue::String(title),
        media,
        category,
        authors,
        media_tags,
        tags,
    ] = row
    else {
        return Err(format!("{row:?}"));
    };
    let category = text_or_null(category)?
        .map(|text| category_from(&text))
        .transpose()?;
    Ok(DocumentNode {
        id: id.parse().map_err(|_| format!("{id:?}"))?,
        title,
        labels: DocumentLabels {
            media: text_or_null(media)?,
            category,
            authors: text_list(authors)?,
            media_tags: tag_set(media_tags)?,
            tags: tag_set(tags)?,
        },
    })
}

fn media_from_row(row: Vec<FalkorValue>) -> Result<MediaNode, String> {
    let row = <[FalkorValue; 4]>::try_from(row).map_err(|row| format!("{row:?}"))?;
    let [
        FalkorValue::String(title),
        FalkorValue::String(category),
        authors,
        tags,
    ] = row
    else {
        return Err(format!("{row:?}"));
    };
    Ok(MediaNode {
        title,
        labels: MediaLabels {
            category: category_from(&category)?,
            authors: text_list(authors)?,
            tags: tag_set(tags)?,
        },
    })
}

fn category_from(text: &str) -> Result<Category, String> {
    text.parse().map_err(|_| format!("{text:?}"))
}

fn text_or_null(value: FalkorValue) -> Result<Option<String>, String> {
    match value {
        FalkorValue::None => Ok(None),
        FalkorValue::String(text) => Ok(Some(text)),
        other => Err(format!("{other:?}")),
    }
}

fn text_list(value: FalkorValue) -> Result<Vec<String>, String> {
    let texts = match value {
        FalkorValue::None => return Ok(Vec::new()),
        FalkorValue::Array(texts) => texts,
        other => return Err(format!("{other:?}")),
    };
    texts
        .into_iter()
        .map(|text| match text {
            FalkorValue::String(text) => Ok(text),
            other => Err(format!("{other:?}")),
        })
        .collect()
}

fn tag_set(value: FalkorValue) -> Result<BTreeSet<Tag>, String> {
    let tags = match value {
        FalkorValue::None => return Ok(BTreeSet::new()),
        FalkorValue::Array(tags) => tags,
        other => return Err(format!("{other:?}")),
    };
    tags.into_iter()
        .map(|tag| match tag {
            FalkorValue::String(text) => text.parse::<Tag>().map_err(|_| format!("{text:?}")),
            other => Err(format!("{other:?}")),
        })
        .collect()
}

fn ingested_items_from_row(row: Vec<FalkorValue>) -> Result<Option<u64>, String> {
    let row = <[FalkorValue; 1]>::try_from(row).map_err(|row| format!("{row:?}"))?;
    match row {
        [FalkorValue::None] => Ok(None),
        [FalkorValue::I64(count)] => u64::try_from(count)
            .map(Some)
            .map_err(|_| count.to_string()),
        other => Err(format!("{other:?}")),
    }
}

fn folder_from_row(row: Vec<FalkorValue>) -> Result<(DocId, PathBuf), String> {
    let row = <[FalkorValue; 2]>::try_from(row).map_err(|row| format!("{row:?}"))?;
    let [FalkorValue::String(id), FalkorValue::String(folder)] = row else {
        return Err(format!("{row:?}"));
    };
    let id = id.parse().map_err(|_| format!("{id:?}"))?;
    Ok((id, PathBuf::from(folder)))
}
