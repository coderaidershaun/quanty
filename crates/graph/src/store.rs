//! What the graph holds, and what a program can ask of the store that holds it.

use std::str::FromStr;

use falkordb::FalkorDBError;
use rag_core::{ConceptId, DocId, ItemId, ItemKind};

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

    #[error(
        "FalkorDB at {url} answered the request to {action} in the graph {graph} with {found}, which is not {expected}"
    )]
    UnreadableReply {
        url: String,
        graph: String,
        action: &'static str,
        found: String,
        /// What the reply should have been, in words that fit after "which is not".
        expected: &'static str,
    },
}

/// The graph of documents, items and concepts. Every write can be repeated: a second call with
/// the same values changes nothing.
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

    /// Removes the document node, its item nodes and every edge of those nodes, the `MENTIONS`
    /// edges of the items among them. Concepts and their `RELATES_TO` edges stay: they belong to
    /// no document. Returns how many nodes it removed. Zero means the graph has no such document,
    /// which is not an error.
    ///
    /// # Errors
    /// [`GraphError::Query`] when the store refuses or cannot be reached.
    fn delete_document(&self, id: DocId) -> impl Future<Output = Result<u64, GraphError>> + Send;

    /// Creates the concept node with no aliases, or writes its name, normalised name and
    /// definition again. It leaves the aliases of a stored concept as they are. Repeating it
    /// changes nothing.
    ///
    /// # Errors
    /// [`GraphError::Query`] when the store refuses or cannot be reached.
    fn upsert_concept(
        &self,
        concept: &ConceptNode,
    ) -> impl Future<Output = Result<(), GraphError>> + Send;

    /// Writes one `MENTIONS` edge from an item to a concept for each mention, with the wording.
    /// An item mentions a concept once: a second mention of the same pair replaces the wording.
    /// An empty slice makes no call.
    ///
    /// # Errors
    /// [`GraphError::Query`] when the store refuses or cannot be reached.
    fn add_mentions(
        &self,
        mentions: &[Mention],
    ) -> impl Future<Output = Result<(), GraphError>> + Send;

    /// Writes one `RELATES_TO` edge for each relation. Two concepts have at most one edge of a
    /// kind: it keeps the item that stated it first. An empty slice makes no call.
    ///
    /// # Errors
    /// [`GraphError::Query`] when the store refuses or cannot be reached.
    fn add_relations(
        &self,
        relations: &[Relation],
    ) -> impl Future<Output = Result<(), GraphError>> + Send;

    /// Adds the name to the concept. A name is not added when the concept already has its
    /// normalised form, as its name or as an alias, so repeating it changes nothing. A concept
    /// that is not in the graph is not created.
    ///
    /// # Errors
    /// [`GraphError::Query`] when the store refuses or cannot be reached.
    fn add_alias(
        &self,
        alias: &ConceptAlias,
    ) -> impl Future<Output = Result<(), GraphError>> + Send;

    /// The concept that has exactly this text as its normalised name or as one of its normalised
    /// aliases, or `None`. The text is compared as it is given, so a name that is not in its
    /// normalised form finds nothing.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a
    ///   concept
    fn find_concept_by_name(
        &self,
        normalised_name: &str,
    ) -> impl Future<Output = Result<Option<ConceptNode>, GraphError>> + Send;

    /// The concept with this id, or `None`.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a
    ///   concept
    fn concept(
        &self,
        id: ConceptId,
    ) -> impl Future<Output = Result<Option<ConceptNode>, GraphError>> + Send;

    /// The concepts that at least one of the items mentions, each once. They are ordered by how
    /// many of the items mention them, the most first, and then by normalised name. An empty
    /// slice makes no call.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a
    ///   concept
    fn concepts_for_items(
        &self,
        items: &[ItemId],
    ) -> impl Future<Output = Result<Vec<ConceptNode>, GraphError>> + Send;

    /// The items that mention at least one of the concepts, each once. Each item comes with the
    /// concepts it mentions among these. The items are ordered by how many of the concepts they
    /// mention, the most first, and then by id, and at most `limit` of them come back. An empty
    /// slice makes no call.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not an
    ///   item with its concepts
    fn items_for_concepts(
        &self,
        concepts: &[ConceptId],
        limit: usize,
    ) -> impl Future<Output = Result<Vec<ItemMentions>, GraphError>> + Send;

    /// The concepts that a `RELATES_TO` edge joins to one of these concepts, in either direction,
    /// each once, by normalised name. A concept that is among the given ones is not in the
    /// answer. An empty slice makes no call.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a
    ///   concept
    fn related_concepts(
        &self,
        concepts: &[ConceptId],
    ) -> impl Future<Output = Result<Vec<ConceptNode>, GraphError>> + Send;
}
