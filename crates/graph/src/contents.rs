//! What the graph holds: the nodes and the edges that a program writes to it and reads from it.

use std::path::PathBuf;
use std::str::FromStr;

use rag_core::{ConceptId, DocId, DocumentLabels, ItemId, ItemKind, MediaLabels};

#[cfg(doc)]
use crate::store::GraphStore;

/// A media a person saved, or that an ingest created. It has no edge: a document names its media
/// in its own labels.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MediaNode {
    pub title: String,
    pub labels: MediaLabels,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DocumentNode {
    pub id: DocId,
    pub title: String,
    pub labels: DocumentLabels,
}

/// Its id is also the id of its point in Qdrant.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ItemNode {
    pub id: ItemId,
    pub kind: ItemKind,
    pub page: u32,
    pub printed_page: Option<String>,
}

/// It belongs to no document. It starts with no aliases, and only [`GraphStore::add_alias`]
/// adds one.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConceptNode {
    pub id: ConceptId,
    /// The name as the first item that named it wrote it.
    pub name: String,
    /// The name in the form that names are compared in. A concept is found by it.
    pub normalised_name: String,
    /// What the concept is, in one line.
    pub definition: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConceptAlias {
    pub concept: ConceptId,
    /// The name as the item wrote it.
    pub name: String,
    /// The name in the form that names are compared in. The concept is found by it.
    pub normalised_name: String,
}

/// An item, and the concepts it mentions among the ones that were asked about.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ItemMentions {
    pub item: ItemId,
    /// In no fixed order.
    pub concepts: Vec<ConceptId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Mention {
    pub item: ItemId,
    pub concept: ConceptId,
    /// The name the item used.
    pub wording: String,
}

/// The enum and `ALL` are written from the one list of names, so a kind cannot be in the enum and
/// missing from `ALL`.
macro_rules! enum_with_all {
    (
        $(#[$attribute:meta])*
        pub enum $name:ident { $($kind:ident),+ $(,)? }
    ) => {
        $(#[$attribute])*
        pub enum $name { $($kind),+ }

        impl $name {
            /// Every kind, in the order of the enum.
            pub const ALL: [$name; [$($name::$kind),+].len()] = [$($name::$kind),+];
        }
    };
}

enum_with_all! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum RelationKind {
        DerivedFrom,
        Assumes,
        Generalises,
        PartOf,
        UsedFor,
    }
}

impl RelationKind {
    /// The name stored on the edge.
    pub fn as_str(self) -> &'static str {
        match self {
            RelationKind::DerivedFrom => "DERIVED_FROM",
            RelationKind::Assumes => "ASSUMES",
            RelationKind::Generalises => "GENERALISES",
            RelationKind::PartOf => "PART_OF",
            RelationKind::UsedFor => "USED_FOR",
        }
    }
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
#[error(
    "`{given}` is not a kind of relation; the kinds are {kinds}",
    kinds = RelationKind::ALL.map(RelationKind::as_str).join(", ")
)]
pub struct UnknownRelationKind {
    given: String,
}

impl FromStr for RelationKind {
    type Err = UnknownRelationKind;

    fn from_str(text: &str) -> Result<RelationKind, UnknownRelationKind> {
        RelationKind::ALL
            .into_iter()
            .find(|kind| kind.as_str() == text)
            .ok_or_else(|| UnknownRelationKind {
                given: text.to_owned(),
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Relation {
    pub from: ConceptId,
    pub to: ConceptId,
    pub kind: RelationKind,
    /// The item that stated it.
    pub item: ItemId,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ItemsByKind {
    pub chunks: u64,
    pub formulas: u64,
    pub figures: u64,
    pub tables: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DocumentRecord {
    pub node: DocumentNode,
    /// The number of items the document was ingested whole with. `None` when an ingest stopped
    /// or skipped an item, and for a document that was stored before the mark existed.
    pub ingested_items: Option<u64>,
    /// The folder the document was ingested from. `None` for a lone picture, and for a document
    /// that was stored before folders were kept.
    pub chapter_folder: Option<PathBuf>,
    pub items: ItemsByKind,
}
