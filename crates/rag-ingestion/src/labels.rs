//! Changes the own tags of a document that is already stored, in both stores, without embedding
//! anything or asking a model.

use std::collections::BTreeSet;
use std::fmt;

use graph::{DocumentNode, GraphError, GraphStore};
use rag_core::{DocId, DocumentLabels, StoreError, Tag};

use crate::stores::Stores;

/// The labels of the media are not here: they are changed on the media, for all its documents.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TagChange {
    /// A tag that the document has is not added twice.
    pub add: Vec<Tag>,
    /// Tags to take away, after the tags to add were added. Taking away a tag that the document
    /// does not have changes nothing.
    pub remove: Vec<Tag>,
}

impl TagChange {
    pub fn is_empty(&self) -> bool {
        self.add.is_empty() && self.remove.is_empty()
    }

    fn apply_to(&self, tags: &mut BTreeSet<Tag>) {
        tags.extend(self.add.iter().cloned());
        for tag in &self.remove {
            tags.remove(tag);
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum RelabelError {
    #[error("could not read or write the labels in the graph")]
    Graph(#[from] GraphError),

    #[error("could not write the labels to the points of the document")]
    Store(#[from] StoreError),

    #[error(
        "the graph holds no document with the id {id}, so no label was changed; use the id that an ingest prints as \"document id\""
    )]
    UnknownDocument { id: DocId },

    #[error(
        "the graph holds no media titled {title:?}, so no label was changed; use the title that the media was saved or ingested with"
    )]
    UnknownMedia { title: String },
}

/// The labels that a document has after [`relabel_document_tags`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relabelled {
    pub doc_id: DocId,
    pub labels: DocumentLabels,
}

impl fmt::Display for Relabelled {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let labels = &self.labels;
        write!(
            formatter,
            "labels written to document {} in both stores",
            self.doc_id
        )?;
        if let Some(media) = &labels.media {
            write!(formatter, "\nmedia: {media}")?;
        }
        if let Some(category) = labels.category {
            write!(formatter, "\ncategory: {category}")?;
        }
        if !labels.authors.is_empty() {
            write!(formatter, "\nauthors: {}", labels.authors.join(", "))?;
        }
        if !labels.media_tags.is_empty() {
            write!(formatter, "\nmedia tags: {}", tag_text(&labels.media_tags))?;
        }
        write!(formatter, "\ntags: {}", tag_text(&labels.tags))
    }
}

/// The tags with commas between them, or "none".
pub(crate) fn tag_text(tags: &BTreeSet<Tag>) -> String {
    if tags.is_empty() {
        return "none".to_owned();
    }
    tags.iter().map(Tag::as_str).collect::<Vec<_>>().join(", ")
}

/// Applies the change to the own tags of the stored document: the graph node first, as an ingest
/// writes it, and then every point of the document. The labels of its media stay as they are. It
/// takes no embedder and no model, so it cannot call one. Running it again with the same change
/// changes nothing.
///
/// # Errors
/// - [`RelabelError::UnknownDocument`] when the graph holds no document with the id
/// - [`RelabelError::Graph`] and [`RelabelError::Store`] when a store fails
pub async fn relabel_document_tags<G: GraphStore>(
    id: DocId,
    change: &TagChange,
    stores: &Stores<G>,
) -> Result<Relabelled, RelabelError> {
    let mut node = stored_node(id, stores)
        .await?
        .ok_or(RelabelError::UnknownDocument { id })?;
    change.apply_to(&mut node.labels.tags);
    stores.graph.upsert_document(&node).await?;
    stores.items.set_document_labels(id, &node.labels).await?;
    Ok(Relabelled {
        doc_id: id,
        labels: node.labels,
    })
}

pub(crate) async fn stored_node<G: GraphStore>(
    id: DocId,
    stores: &Stores<G>,
) -> Result<Option<DocumentNode>, GraphError> {
    stores.graph.document(id).await
}
