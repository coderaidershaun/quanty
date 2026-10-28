//! Everything that can stop a tool that explores the concept graph: an argument that is refused,
//! a concept, document or item that is not stored, and a store that is down.

use graph::{GraphError, UnknownRelationKind};
use rag_core::{ConceptId, EmbedError, ItemId, StoreError, UnknownItemKind};

use crate::library::LibraryError;

#[derive(thiserror::Error, Debug)]
pub(crate) enum ConceptsError {
    #[error(
        "`{argument}` is not a concept id; give the `concept_id` that a result gave, or call `find_concepts` to look one up"
    )]
    NotAConceptId {
        argument: &'static str,
        #[source]
        source: uuid::Error,
    },

    #[error("`{argument}` is blank; give a concept id or the name of a concept")]
    BlankConcept { argument: &'static str },

    #[error(
        "give `concept_id` or `name`, not both; or give neither for the most mentioned concepts"
    )]
    BothConcepts,

    #[error(
        "give exactly one of `name` and `query`: `name` finds a concept by its exact name or alias and is free, `query` finds concepts by meaning and is PAID"
    )]
    FindArguments,

    #[error(
        "no stored concept has the id {id}; call `find_concepts` to find a concept and its `concept_id`"
    )]
    UnknownConceptId { id: ConceptId },

    #[error(
        "no stored concept is named {name:?}, nor has it as an alias; call `find_concepts` with a `query` to search by meaning"
    )]
    UnknownConceptName { name: String },

    #[error("`from` and `to` are the same concept, {name}; give two different concepts")]
    SameConcept { name: String },

    #[error("`{argument}` is {given}; give a whole number from {least} to {most}")]
    OutOfRange {
        argument: &'static str,
        given: u32,
        least: u32,
        most: u32,
    },

    #[error("`{argument}` is {given:?}; give {allowed}, or leave it out")]
    NotOneOf {
        argument: &'static str,
        given: String,
        allowed: &'static str,
    },

    #[error("`kinds` is not valid")]
    RelationKind(#[source] UnknownRelationKind),

    #[error("`kind` is not valid")]
    ItemKind(#[source] UnknownItemKind),

    #[error(
        "`cursor` is {given:?}, which no result gave; give the `next_cursor` of the previous result, or leave it out"
    )]
    Cursor { given: String },

    #[error(
        "`document_ids` has {given} entries and {distinct} of them are different; give from 2 to 10 different document ids"
    )]
    DocumentCount { given: usize, distinct: usize },

    #[error("`item_id` is not an item id; give the `item_id` that a result gave")]
    NotAnItemId(#[source] uuid::Error),

    #[error(
        "no stored item has the id {id}; take the `item_id` from a result of `search`, `concept_items` or `similar_items`"
    )]
    UnknownItem { id: ItemId },

    #[error(transparent)]
    Library(#[from] LibraryError),

    #[error("could not read the concept graph")]
    Graph(#[from] GraphError),

    #[error("could not set up the embedder")]
    Embedder(#[from] EmbedError),

    #[error("could not read the item store")]
    ItemStore(#[source] StoreError),

    #[error("could not read the concept store")]
    ConceptStore(#[source] StoreError),
}

impl ConceptsError {
    /// Whether a store or the embedder is the cause, so that the text of the failure points at
    /// `health`.
    pub(crate) fn is_service_failure(&self) -> bool {
        match self {
            ConceptsError::Library(error) => error.is_service_failure(),
            ConceptsError::Graph(_)
            | ConceptsError::Embedder(_)
            | ConceptsError::ItemStore(_)
            | ConceptsError::ConceptStore(_) => true,
            ConceptsError::NotAConceptId { .. }
            | ConceptsError::BlankConcept { .. }
            | ConceptsError::BothConcepts
            | ConceptsError::FindArguments
            | ConceptsError::UnknownConceptId { .. }
            | ConceptsError::UnknownConceptName { .. }
            | ConceptsError::SameConcept { .. }
            | ConceptsError::OutOfRange { .. }
            | ConceptsError::NotOneOf { .. }
            | ConceptsError::RelationKind(_)
            | ConceptsError::ItemKind(_)
            | ConceptsError::Cursor { .. }
            | ConceptsError::DocumentCount { .. }
            | ConceptsError::NotAnItemId(_)
            | ConceptsError::UnknownItem { .. } => false,
        }
    }
}
