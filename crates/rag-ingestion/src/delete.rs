//! Removes one document from both stores: its points from Qdrant and its nodes from the graph.

use std::fmt;

use graph::{GraphError, GraphStore};
use rag_core::{DocId, StoreError};

use crate::stores::Stores;

#[derive(thiserror::Error, Debug)]
pub enum DeleteError {
    #[error("could not remove the points of the document")]
    Store(#[from] StoreError),

    #[error("could not remove the nodes of the document")]
    Graph(#[from] GraphError),

    #[error(
        "nothing is stored under the document id {id} in the collection {collection} or in the graph, so nothing was removed; use the id that an ingest prints as \"document id\""
    )]
    UnknownDocument { id: DocId, collection: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteSummary {
    pub doc_id: DocId,
    pub collection: String,
    pub points_removed: u64,
    /// The document node and its item nodes.
    pub nodes_removed: u64,
}

impl fmt::Display for DeleteSummary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(formatter, "document id: {}", self.doc_id)?;
        writeln!(
            formatter,
            "points removed from collection {}: {}",
            self.collection, self.points_removed
        )?;
        write!(
            formatter,
            "nodes removed from the graph: {} (the document and its items)",
            self.nodes_removed
        )
    }
}

/// Removes the points and then the nodes of the document, the reverse of the order that ingest
/// writes them in. A run that stops in the middle leaves what a half-finished ingest leaves, and
/// running it again removes the rest. A store that holds nothing of the document does not stop
/// the job, so that a second run can remove what the first one did not reach.
///
/// # Errors
/// - [`DeleteError::Store`] and [`DeleteError::Graph`] when a store fails. The first failure
///   stops the job, so when Qdrant fails the graph is not asked.
/// - [`DeleteError::UnknownDocument`] when neither store held anything under the id
pub async fn delete_document<G: GraphStore>(
    id: DocId,
    stores: &Stores<G>,
) -> Result<DeleteSummary, DeleteError> {
    let points_removed = stores.items.delete_document(id).await?;
    let nodes_removed = stores.graph.delete_document(id).await?;
    let collection = stores.items.collection().to_owned();
    if points_removed == 0 && nodes_removed == 0 {
        return Err(DeleteError::UnknownDocument { id, collection });
    }
    Ok(DeleteSummary {
        doc_id: id,
        collection,
        points_removed,
        nodes_removed,
    })
}
