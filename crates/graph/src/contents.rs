//! What the graph holds: the nodes and the edges that a program writes to it and reads from it.

use std::path::PathBuf;
use std::str::FromStr;

use rag_core::{ConceptId, DocId, DocumentLabels, ItemId, ItemKind};

#[cfg(doc)]
use crate::store::GraphStore;

/// A document as the graph holds it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DocumentNode {
    pub id: DocId,
    pub title: String,
    pub labels: DocumentLabels,
}

/// An item as the graph holds it. Its id is also the id of its point in Qdrant.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ItemNode {
    pub id: ItemId,
    pub kind: ItemKind,
    pub page: u32,
    pub printed_page: Option<String>,
}

/// A concept as the graph holds it. It belongs to no document. It starts with no aliases, and
/// only [`GraphStore::add_alias`] adds one.
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

/// One more name for a stored concept.
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

/// An item discusses a concept.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Mention {
    pub item: ItemId,
    pub concept: ConceptId,
    /// The name the item used.
    pub wording: String,
}

/// How one concept relates to another. The list is fixed: a new kind is a change to this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelationKind {
    DerivedFrom,
    Assumes,
    Generalises,
    PartOf,
    UsedFor,
}

impl RelationKind {
    /// Every kind, in the order of the enum.
    // SMELL: a kind that is added to the enum must be added to this list by hand. Nothing checks
    // it, and a kind that is missing here is refused as unknown.
    pub const ALL: [RelationKind; 5] = [
        RelationKind::DerivedFrom,
        RelationKind::Assumes,
        RelationKind::Generalises,
        RelationKind::PartOf,
        RelationKind::UsedFor,
    ];

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

/// The text that was given as a kind of relation is not the name of any kind.
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

/// One concept relates to another.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Relation {
    pub from: ConceptId,
    pub to: ConceptId,
    pub kind: RelationKind,
    /// The item that stated it.
    pub item: ItemId,
}

/// How many items of each kind a document holds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ItemsByKind {
    pub chunks: u64,
    pub formulas: u64,
    pub figures: u64,
    pub tables: u64,
}

/// A document with everything the graph keeps about it.
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
