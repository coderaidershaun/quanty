//! Talks to the FalkorDB graph store. Test support for other crates is in `graph::testing`,
//! behind the cargo feature `testing`.

mod contents;
mod falkor;
mod store;
#[cfg(feature = "testing")]
pub mod testing;

pub use contents::{
    ConceptAlias, ConceptNode, DocumentNode, DocumentRecord, ItemMentions, ItemNode, ItemsByKind,
    Mention, Relation, RelationKind, UnknownRelationKind,
};
pub use falkor::FalkorGraph;
pub use store::{GraphError, GraphStore};
