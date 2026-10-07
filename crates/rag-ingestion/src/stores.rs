//! The stores that a document is written to and removed from, so a job takes one value for all
//! of them.

use rag_core::{ConceptStore, ItemStore};

/// The stores a document is written to: its points, its nodes and the vectors of its concepts. A
/// concept belongs to no document, so removing a document leaves the concept store alone.
pub struct Stores<G> {
    pub items: ItemStore,
    pub graph: G,
    pub concepts: ConceptStore,
}
