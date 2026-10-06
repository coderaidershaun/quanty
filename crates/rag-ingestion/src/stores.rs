//! The two stores that a document is written to and removed from, so a job takes one value for
//! both.

use rag_core::ItemStore;

/// The two stores a document is written to: its points and its nodes.
pub struct Stores<G> {
    pub items: ItemStore,
    pub graph: G,
}
