//! Decides which stored concept an extracted concept belongs to: the one with the same
//! normalised name, or a new one.

use graph::{ConceptNode, GraphError, GraphStore};
use rag_core::ConceptId;

use super::question::ExtractedConcept;

pub(super) enum Resolved {
    Created(ConceptId),
    Linked(ConceptId),
}

/// The form in which names are compared: lower case, with every run of characters that are not
/// letters or digits turned into one space. A hyphen is a word break, not a letter to delete, so
/// "Black–Scholes", "black-scholes" and "Black Scholes" are the same name.
pub(super) fn normalised(name: &str) -> String {
    let mut normalised = String::new();
    let mut after_break = false;
    for character in name.to_lowercase().chars() {
        if !character.is_alphanumeric() {
            after_break = true;
            continue;
        }
        if after_break && !normalised.is_empty() {
            normalised.push(' ');
        }
        after_break = false;
        normalised.push(character);
    }
    normalised
}

// SMELL: finding a concept and creating it are two calls, so two ingests that run at the same
// time can each create the same concept.
/// The stored concept with the same normalised name as the extracted one. When there is none, a
/// new concept is written and its id returned.
///
/// # Errors
/// [`GraphError`] when the graph cannot be read or written.
pub(super) async fn resolve<G: GraphStore>(
    graph: &G,
    concept: &ExtractedConcept,
) -> Result<Resolved, GraphError> {
    let normalised_name = normalised(&concept.name);
    if let Some(id) = graph.find_concept_by_name(&normalised_name).await? {
        return Ok(Resolved::Linked(id));
    }
    let id = ConceptId::random();
    graph
        .upsert_concept(&ConceptNode {
            id,
            name: concept.name.clone(),
            normalised_name,
            definition: concept.definition.clone(),
            aliases: Vec::new(),
        })
        .await?;
    Ok(Resolved::Created(id))
}
