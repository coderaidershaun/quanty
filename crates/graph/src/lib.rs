//! Talks to the FalkorDB graph store. Test support for other crates is in `graph::testing`,
//! behind the cargo feature `testing`.

mod contents;
mod error;
mod falkor;
#[cfg(feature = "testing")]
pub mod testing;

pub use contents::{
    ConceptAlias, ConceptDocument, ConceptItem, ConceptNode, Direction, DocumentNode,
    DocumentRecord, ItemMentions, ItemNode, ItemPlace, ItemsByKind, MediaNode, Mention, PathDepth,
    Relation, RelationKind, SharedConcept, UnknownRelationKind,
};
pub use error::GraphError;
pub use falkor::FalkorGraph;
