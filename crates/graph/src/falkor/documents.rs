//! The statements that read documents and media, whether a document is ingested whole and where
//! it was ingested from, with the code that runs each one and the row it expects.

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

use falkordb::FalkorValue;
use rag_core::{DocId, DocumentLabels, MediaLabels, Tag};

use super::{
    FalkorGraph, fixed, id_list, id_value, parsed, read_rows, texts_into, unreadable_reply,
};
use crate::contents::{DocumentNode, MediaNode};
use crate::error::GraphError;

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

impl FalkorGraph {
    /// Every document with its title and its labels, ordered by id.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a
    ///   document
    pub async fn documents(&self) -> Result<Vec<DocumentNode>, GraphError> {
        let action = "read the documents";
        let reply = self.run(action, &documents_statement(), Vec::new()).await?;
        read_rows(self, action, DOCUMENT_ROW, reply, document_from_row)
    }

    /// The document with this id, with its title and its labels, or `None`.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a
    ///   document
    pub async fn document(&self, id: DocId) -> Result<Option<DocumentNode>, GraphError> {
        let action = "read the document";
        let parameters = vec![("id", id_value(id))];
        let reply = self.run(action, &document_statement(), parameters).await?;
        reply
            .data
            .into_values_lossy()
            .next()
            .map(document_from_row)
            .transpose()
            .map_err(|found| unreadable_reply(self, action, DOCUMENT_ROW, found))
    }

    /// Every media, ordered by title. A media title that is only a label on documents is not one.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a
    ///   media
    pub async fn media(&self) -> Result<Vec<MediaNode>, GraphError> {
        let action = "read the media";
        let reply = self.run(action, MEDIA, Vec::new()).await?;
        read_rows(self, action, MEDIA_ROW, reply, media_from_row)
    }

    /// The number of items that the document is marked as ingested whole with, or `None` when it
    /// has no mark or is not in the graph.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a
    ///   count of items
    pub async fn ingested_items(&self, document: DocId) -> Result<Option<u64>, GraphError> {
        let action = "read how many items the document was ingested with";
        let parameters = vec![("id", id_value(document))];
        let reply = self.run(action, INGESTED_ITEMS, parameters).await?;
        match reply.data.into_values_lossy().next() {
            None => Ok(None),
            Some(row) => ingested_items_from_row(row)
                .map_err(|found| unreadable_reply(self, action, INGESTED_ITEMS_ROW, found)),
        }
    }

    /// The folder that each of these documents was ingested from. A document that is not in the
    /// graph, or that has no folder, is left out. An empty slice makes no call.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a
    ///   document with its folder
    pub async fn chapter_folders(
        &self,
        documents: &[DocId],
    ) -> Result<HashMap<DocId, PathBuf>, GraphError> {
        if documents.is_empty() {
            return Ok(HashMap::new());
        }
        let action = "read the folders the documents were ingested from";
        let parameters = vec![("documents", id_list(documents))];
        let reply = self.run(action, CHAPTER_FOLDERS, parameters).await?;
        read_rows(self, action, FOLDER_ROW, reply, folder_from_row)
    }
}

pub(super) fn document_from_row(row: Vec<FalkorValue>) -> Result<DocumentNode, String> {
    let row = fixed::<7>(row)?;
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
        .map(|text| parsed(&text))
        .transpose()?;
    Ok(DocumentNode {
        id: parsed(&id)?,
        title,
        labels: DocumentLabels {
            media: text_or_null(media)?,
            category,
            authors: texts_into(authors, Ok)?,
            media_tags: tag_set(media_tags)?,
            tags: tag_set(tags)?,
        },
    })
}

fn media_from_row(row: Vec<FalkorValue>) -> Result<MediaNode, String> {
    let row = fixed::<4>(row)?;
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
            category: parsed(&category)?,
            authors: texts_into(authors, Ok)?,
            tags: tag_set(tags)?,
        },
    })
}

pub(super) fn text_or_null(value: FalkorValue) -> Result<Option<String>, String> {
    match value {
        FalkorValue::None => Ok(None),
        FalkorValue::String(text) => Ok(Some(text)),
        other => Err(format!("{other:?}")),
    }
}

/// The graph stores a count as a signed number or null, so a negative count is an error.
pub(super) fn count_or_null(value: FalkorValue) -> Result<Option<u64>, String> {
    match value {
        FalkorValue::None => Ok(None),
        FalkorValue::I64(count) => u64::try_from(count)
            .map(Some)
            .map_err(|_| count.to_string()),
        other => Err(format!("{other:?}")),
    }
}

fn tag_set(value: FalkorValue) -> Result<BTreeSet<Tag>, String> {
    texts_into(value, |text| parsed(&text))
}

fn ingested_items_from_row(row: Vec<FalkorValue>) -> Result<Option<u64>, String> {
    let row = fixed::<1>(row)?;
    match row {
        [value @ (FalkorValue::None | FalkorValue::I64(_))] => count_or_null(value),
        other => Err(format!("{other:?}")),
    }
}

fn folder_from_row(row: Vec<FalkorValue>) -> Result<(DocId, PathBuf), String> {
    let row = fixed::<2>(row)?;
    let [FalkorValue::String(id), FalkorValue::String(folder)] = row else {
        return Err(format!("{row:?}"));
    };
    let id = parsed(&id)?;
    Ok((id, PathBuf::from(folder)))
}
