//! Changes the labels of a document that is already stored, in both stores, without embedding
//! anything or asking a model.

use std::fmt;

use graph::{DocumentNode, GraphError, GraphStore};
use rag_core::{DocId, DocumentLabels, StoreError, Tag};

use crate::stores::Stores;

/// The book is not here: it comes from the chapter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LabelChange {
    /// The new author, or `None` to leave the author as it is.
    pub author: Option<String>,
    /// A tag that the document has is not added twice.
    pub add: Vec<Tag>,
    /// Tags to take away, after the tags to add were added. Taking away a tag that the document
    /// does not have changes nothing.
    pub remove: Vec<Tag>,
}

impl LabelChange {
    pub fn is_empty(&self) -> bool {
        self.author.is_none() && self.add.is_empty() && self.remove.is_empty()
    }

    fn apply_to(&self, labels: &mut DocumentLabels) {
        if let Some(author) = &self.author {
            labels.author = Some(author.clone());
        }
        labels.tags.extend(self.add.iter().cloned());
        for tag in &self.remove {
            labels.tags.remove(tag);
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum RelabelError {
    #[error("could not read or write the document in the graph")]
    Graph(#[from] GraphError),

    #[error("could not write the labels to the points of the document")]
    Store(#[from] StoreError),

    #[error(
        "the graph holds no document with the id {id}, so no label was changed; use the id that an ingest prints as \"document id\""
    )]
    UnknownDocument { id: DocId },
}

/// The labels that a document has after [`relabel`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relabelled {
    pub doc_id: DocId,
    pub labels: DocumentLabels,
}

impl fmt::Display for Relabelled {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "labels written to document {} in both stores",
            self.doc_id
        )?;
        if let Some(book) = &self.labels.book {
            write!(formatter, "\nbook: {book}")?;
        }
        if let Some(author) = &self.labels.author {
            write!(formatter, "\nauthor: {author}")?;
        }
        if self.labels.tags.is_empty() {
            write!(formatter, "\ntags: none")
        } else {
            let tags: Vec<&str> = self.labels.tags.iter().map(Tag::as_str).collect();
            write!(formatter, "\ntags: {}", tags.join(", "))
        }
    }
}

/// Applies the change to the labels of the stored document: the graph node first, as an ingest
/// writes it, and then every point of the document. It takes no embedder and no model, so it
/// cannot call one. Running it again with the same change changes nothing.
///
/// # Errors
/// - [`RelabelError::UnknownDocument`] when the graph holds no document with the id
/// - [`RelabelError::Graph`] and [`RelabelError::Store`] when a store fails
pub async fn relabel<G: GraphStore>(
    id: DocId,
    change: &LabelChange,
    stores: &Stores<G>,
) -> Result<Relabelled, RelabelError> {
    let mut node = stored_node(id, stores)
        .await?
        .ok_or(RelabelError::UnknownDocument { id })?;
    change.apply_to(&mut node.labels);
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
    let documents = stores.graph.documents().await?;
    Ok(documents.into_iter().find(|node| node.id == id))
}
