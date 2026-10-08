//! What a program can ask of the graph store, and the ways a request can fail.

use std::path::{Path, PathBuf};

use falkordb::FalkorDBError;
use rag_core::{ConceptId, DocId, ItemId};

use crate::contents::{
    ConceptAlias, ConceptNode, DocumentNode, DocumentRecord, ItemMentions, ItemNode, MediaNode,
    Mention, Relation,
};

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

    #[error(
        "the folder {} cannot be kept in the graph because its path is not valid Unicode",
        folder.display()
    )]
    FolderNotUnicode { folder: PathBuf },
}

/// Every write can be repeated: a second call with the same values changes nothing.
pub trait GraphStore {
    /// Creates the document node, or updates its title and its labels: a label that `document`
    /// does not have is taken away from the stored node. Repeating it changes nothing.
    ///
    /// # Errors
    /// [`GraphError::Query`] when the store refuses or cannot be reached.
    fn upsert_document(
        &self,
        document: &DocumentNode,
    ) -> impl Future<Output = Result<(), GraphError>> + Send;

    /// Every document with its title and its labels, ordered by id.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a
    ///   document
    fn documents(&self) -> impl Future<Output = Result<Vec<DocumentNode>, GraphError>> + Send;

    /// Every document, each once, ordered by title and then by id, with its labels, the mark that
    /// it is ingested whole, the folder it was ingested from and how many items of each kind it
    /// has. A document with no items has zero of each kind. This read counts every item in the
    /// graph, so [`GraphStore::documents`] is the read to use when the title and the labels are
    /// enough.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a
    ///   document with its mark and its folder, or not a count of items of one kind
    fn document_records(
        &self,
    ) -> impl Future<Output = Result<Vec<DocumentRecord>, GraphError>> + Send;

    /// Creates the media node. A media with exactly this title that is in the graph stays as it
    /// is, with the labels it has, so an ingest never changes a stored media and repeating it
    /// changes nothing. Titles are compared as they are given: the caller decides whether two
    /// spellings name one media.
    ///
    /// # Errors
    /// [`GraphError::Query`] when the store refuses or cannot be reached.
    fn add_media(&self, media: &MediaNode) -> impl Future<Output = Result<(), GraphError>> + Send;

    /// Creates the media node, or writes its category, its authors and its tags over the ones it
    /// has. Titles are compared as they are given.
    ///
    /// # Errors
    /// [`GraphError::Query`] when the store refuses or cannot be reached.
    fn update_media(
        &self,
        media: &MediaNode,
    ) -> impl Future<Output = Result<(), GraphError>> + Send;

    /// Every media, ordered by title. A media title that is only a label on documents is not one.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a
    ///   media
    fn media(&self) -> impl Future<Output = Result<Vec<MediaNode>, GraphError>> + Send;

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

    /// Records whether the document is ingested whole. `Some(items)` marks it, with the number of
    /// its items. `None` takes the mark away. A document that is not in the graph is not created.
    /// Repeating it changes nothing.
    ///
    /// # Errors
    /// [`GraphError::Query`] when the store refuses or cannot be reached.
    fn set_ingested_items(
        &self,
        document: DocId,
        items: Option<u64>,
    ) -> impl Future<Output = Result<(), GraphError>> + Send;

    /// The number of items that the document is marked as ingested whole with, or `None` when it
    /// has no mark or is not in the graph.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a
    ///   count of items
    fn ingested_items(
        &self,
        document: DocId,
    ) -> impl Future<Output = Result<Option<u64>, GraphError>> + Send;

    /// Records the folder the document was ingested from. A document that is not in the graph is
    /// not created. Repeating it changes nothing.
    ///
    /// # Errors
    /// - [`GraphError::FolderNotUnicode`] when the path of the folder is not valid Unicode, which
    ///   the graph cannot keep
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    fn set_chapter_folder(
        &self,
        document: DocId,
        folder: &Path,
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

    /// The concepts that the items on this page of this document mention, each once. They are
    /// ordered by how many of those items mention them, the most first, and then by normalised
    /// name. `page` is the position of the page in the chapter, counted from 1. A page with no
    /// item, and a document that is not in the graph, give no concept.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a
    ///   concept
    fn concepts_on_page(
        &self,
        document: DocId,
        page: u32,
    ) -> impl Future<Output = Result<Vec<ConceptNode>, GraphError>> + Send;

    /// Every `RELATES_TO` edge that has both ends among these concepts, with its direction (from
    /// the concept it leaves to the concept it reaches), its kind and the item that stated it. The
    /// edges are ordered by the id of the concept they leave, then by the id of the concept they
    /// reach, then by the name of their kind as the edge stores it. An empty slice makes no call.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a
    ///   relation
    fn relations_among(
        &self,
        concepts: &[ConceptId],
    ) -> impl Future<Output = Result<Vec<Relation>, GraphError>> + Send;

    /// Every `MENTIONS` edge from one of these items to one of these concepts, with its wording,
    /// ordered by the id of the item and then by the id of the concept. An empty slice, for the
    /// items or for the concepts, makes no call.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a
    ///   mention
    fn mentions_between(
        &self,
        items: &[ItemId],
        concepts: &[ConceptId],
    ) -> impl Future<Output = Result<Vec<Mention>, GraphError>> + Send;
}
