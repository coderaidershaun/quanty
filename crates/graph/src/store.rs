//! What the graph holds, and what a program can ask of the store that holds it.

use falkordb::FalkorDBError;
use rag_core::{DocId, ItemId, ItemKind};

/// A document as the graph holds it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DocumentNode {
    pub id: DocId,
    pub title: String,
}

/// An item as the graph holds it. Its id is also the id of its point in Qdrant.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ItemNode {
    pub id: ItemId,
    pub kind: ItemKind,
    pub page: u32,
    pub printed_page: Option<String>,
}

#[derive(thiserror::Error, Debug)]
pub enum GraphError {
    #[error("could not connect to FalkorDB at {url}")]
    Connect {
        url: String,
        #[source]
        source: FalkorDBError,
    },

    #[error("FalkorDB at {url} did not answer a request for its list of graphs")]
    Ping {
        url: String,
        #[source]
        source: FalkorDBError,
    },

    // A plain Redis on the same port answers the request with an error reply, which the client
    // can only report as a reply that is not a list.
    #[error(
        "the server at {url} did not answer like FalkorDB; another program may be using that port"
    )]
    NotFalkorDb {
        url: String,
        #[source]
        source: FalkorDBError,
    },

    #[error("FalkorDB at {url} failed to {action} in the graph {graph}")]
    Query {
        url: String,
        graph: String,
        action: &'static str,
        #[source]
        source: FalkorDBError,
    },
}

/// The graph of documents and items. Every write can be repeated: a second call with the same
/// values changes nothing.
pub trait GraphStore {
    /// Creates the document node, or updates its title. Repeating it changes nothing.
    ///
    /// # Errors
    /// [`GraphError::Query`] when the store refuses or cannot be reached.
    fn upsert_document(
        &self,
        document: &DocumentNode,
    ) -> impl Future<Output = Result<(), GraphError>> + Send;

    /// Writes the items of a document: a node for each item, an edge `HAS_ITEM` from the
    /// document to each item, and an edge `NEXT` from each item to the one after it. Creates the
    /// document node when it is missing. Repeating it adds no node and no edge. An empty slice
    /// makes no call.
    ///
    /// `items` must be every item of the document, in reading order, in one call. A second call
    /// with the rest of the items does not join the two parts with a `NEXT` edge.
    ///
    /// # Errors
    /// [`GraphError::Query`] when the store refuses or cannot be reached.
    fn upsert_items(
        &self,
        document: DocId,
        items: &[ItemNode],
    ) -> impl Future<Output = Result<(), GraphError>> + Send;

    /// Removes the document node, its item nodes and every edge of those nodes. Returns how many
    /// nodes it removed. Zero means the graph has no such document, which is not an error.
    ///
    /// # Errors
    /// [`GraphError::Query`] when the store refuses or cannot be reached.
    fn delete_document(&self, id: DocId) -> impl Future<Output = Result<u64, GraphError>> + Send;
}
