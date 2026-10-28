//! How the concept tools read their arguments: one rule for a concept given by id or by name, for
//! a limit, a cursor, a kind of item and a document, so that every tool refuses the same way. Every
//! check here runs before a store is connected.

use graph::{ConceptNode, FalkorGraph};
use rag_core::{ConceptId, DocId, ItemKind};
use rag_ingestion::normalised_concept_name;

use super::error::ConceptsError;
use crate::args::{non_blank, parse_optional};
use crate::library::{LibraryError, parse_document_id};

/// A concept as an agent names it: by the id a result gave, or by a name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConceptRef {
    Id(ConceptId),
    /// The name in the form that names are compared in.
    Name(String),
}

impl ConceptRef {
    /// Text that is a concept id is an id, and any other text is a name.
    ///
    /// # Errors
    /// [`ConceptsError::BlankConcept`] when the text has no letter or digit.
    pub(crate) fn id_or_name(
        argument: &'static str,
        text: &str,
    ) -> Result<ConceptRef, ConceptsError> {
        if let Ok(id) = text.trim().parse() {
            return Ok(ConceptRef::Id(id));
        }
        ConceptRef::named(argument, text)
    }

    /// # Errors
    /// [`ConceptsError::BlankConcept`] when the text has no letter or digit.
    pub(crate) fn named(argument: &'static str, text: &str) -> Result<ConceptRef, ConceptsError> {
        let name = normalised_concept_name(text);
        if name.is_empty() {
            return Err(ConceptsError::BlankConcept { argument });
        }
        Ok(ConceptRef::Name(name))
    }

    /// The stored concept that this names.
    ///
    /// # Errors
    /// - [`ConceptsError::UnknownConceptId`] and [`ConceptsError::UnknownConceptName`] when no
    ///   stored concept has that id or that name or alias
    /// - [`ConceptsError::Graph`] when the graph cannot be read
    pub(crate) async fn find(&self, graph: &FalkorGraph) -> Result<ConceptNode, ConceptsError> {
        match self {
            ConceptRef::Id(id) => graph
                .concept(*id)
                .await?
                .ok_or(ConceptsError::UnknownConceptId { id: *id }),
            ConceptRef::Name(name) => graph
                .find_concept_by_name(name)
                .await?
                .ok_or_else(|| ConceptsError::UnknownConceptName { name: name.clone() }),
        }
    }
}

/// # Errors
/// [`ConceptsError::NotAConceptId`] when the text is not a concept id.
pub(super) fn parse_concept_id(
    argument: &'static str,
    text: &str,
) -> Result<ConceptId, ConceptsError> {
    text.trim()
        .parse()
        .map_err(|source| ConceptsError::NotAConceptId { argument, source })
}

/// The stored concept with this id.
///
/// # Errors
/// [`ConceptsError::UnknownConceptId`] when there is none, [`ConceptsError::Graph`] when the graph
/// cannot be read.
pub(super) async fn stored_concept(
    graph: &FalkorGraph,
    id: ConceptId,
) -> Result<ConceptNode, ConceptsError> {
    ConceptRef::Id(id).find(graph).await
}

/// A whole number the agent gave, or `default` when it gave none.
///
/// # Errors
/// [`ConceptsError::OutOfRange`] when the number is not in `range`.
pub(super) fn bounded(
    argument: &'static str,
    given: Option<u32>,
    range: std::ops::RangeInclusive<u32>,
    default: u32,
) -> Result<usize, ConceptsError> {
    let value = given.unwrap_or(default);
    if !range.contains(&value) {
        return Err(ConceptsError::OutOfRange {
            argument,
            given: value,
            least: *range.start(),
            most: *range.end(),
        });
    }
    Ok(value as usize)
}

/// The place a page of rows starts at: the `next_cursor` of the previous result, or 0.
///
/// # Errors
/// [`ConceptsError::Cursor`] when the text is not a cursor that a result gave.
pub(super) fn offset_of(cursor: Option<String>) -> Result<usize, ConceptsError> {
    match non_blank(cursor) {
        None => Ok(0),
        Some(text) => text
            .parse()
            .map_err(|_| ConceptsError::Cursor { given: text }),
    }
}

/// # Errors
/// [`ConceptsError::ItemKind`] when the text is not a kind of item.
pub(super) fn parse_item_kind(kind: Option<String>) -> Result<Option<ItemKind>, ConceptsError> {
    parse_optional(kind).map_err(ConceptsError::ItemKind)
}

/// The document id that an agent gave.
///
/// # Errors
/// [`ConceptsError::Library`] when the text is not a document id.
pub(super) fn parse_document(text: &str) -> Result<DocId, ConceptsError> {
    Ok(parse_document_id(text)?)
}

/// The title of the stored document.
///
/// # Errors
/// [`ConceptsError::Library`] when the graph holds no such document, [`ConceptsError::Graph`] when
/// it cannot be read.
pub(super) async fn stored_title(graph: &FalkorGraph, id: DocId) -> Result<String, ConceptsError> {
    let document = graph
        .document(id)
        .await?
        .ok_or(LibraryError::NotStored { id })?;
    Ok(document.title)
}
