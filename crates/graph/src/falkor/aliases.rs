//! The statements that read the aliases of concepts, of one or of many.

use std::collections::HashMap;

use falkordb::FalkorValue;
use rag_core::ConceptId;

use super::{
    FalkorGraph, fixed, id_list, id_value, parsed, read_rows, texts_into, unreadable_reply,
};
use crate::error::GraphError;

const ALIASES_ROW: &str = "the aliases of a concept (a list of names or null)";
const CONCEPT_ALIASES_ROW: &str =
    "a concept with its aliases (a concept id and a list of names or null)";

const ALIASES: &str = "\
MATCH (c:Concept {id: $id})
RETURN c.aliases";

const CONCEPT_ALIASES: &str = "\
MATCH (c:Concept)
WHERE c.id IN $concepts
RETURN c.id, c.aliases";

impl FalkorGraph {
    /// The names that items gave the concept besides its own, as the items wrote them, in the
    /// order they were added. A concept that is not in the graph, and a concept with no alias,
    /// give none.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a list
    ///   of names
    pub async fn aliases_of(&self, concept: ConceptId) -> Result<Vec<String>, GraphError> {
        let action = "read the aliases of the concept";
        let parameters = vec![("id", id_value(concept))];
        let reply = self.run(action, ALIASES, parameters).await?;
        let aliases = reply
            .data
            .into_values_lossy()
            .next()
            .map(aliases_from_row)
            .transpose()
            .map_err(|found| unreadable_reply(self, action, ALIASES_ROW, found))?;
        Ok(aliases.unwrap_or_default())
    }

    /// The aliases of each of these concepts, as [`FalkorGraph::aliases_of`] gives them. A concept
    /// that the graph holds and that has no alias has an empty list, and an id that the graph does
    /// not hold is left out. An empty slice makes no call.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a
    ///   concept with a list of names
    pub async fn concept_aliases(
        &self,
        concepts: &[ConceptId],
    ) -> Result<HashMap<ConceptId, Vec<String>>, GraphError> {
        if concepts.is_empty() {
            return Ok(HashMap::new());
        }
        let action = "read the aliases of the concepts";
        let parameters = vec![("concepts", id_list(concepts))];
        let reply = self.run(action, CONCEPT_ALIASES, parameters).await?;
        read_rows(
            self,
            action,
            CONCEPT_ALIASES_ROW,
            reply,
            concept_aliases_from_row,
        )
    }
}

fn aliases_from_row(row: Vec<FalkorValue>) -> Result<Vec<String>, String> {
    let [aliases] = fixed::<1>(row)?;
    texts_into(aliases, Ok)
}

fn concept_aliases_from_row(row: Vec<FalkorValue>) -> Result<(ConceptId, Vec<String>), String> {
    let row = fixed::<2>(row)?;
    let [FalkorValue::String(concept), aliases] = row else {
        return Err(format!("{row:?}"));
    };
    Ok((parsed(&concept)?, texts_into(aliases, Ok)?))
}
