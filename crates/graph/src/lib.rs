//! Talks to the FalkorDB graph store. Test support for other crates is in `graph::testing`,
//! behind the cargo feature `testing`.

mod falkor;
mod store;
#[cfg(feature = "testing")]
pub mod testing;

pub use falkor::FalkorGraph;
pub use store::{
    ConceptNode, DocumentNode, GraphError, GraphStore, ItemNode, Mention, Relation, RelationKind,
    UnknownRelationKind,
};
