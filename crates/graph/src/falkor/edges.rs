//! The reads of the edges that join the concepts a caller draws: the `RELATES_TO` edges among
//! concepts, and the `MENTIONS` edges from items to concepts.

use falkordb::FalkorValue;
use rag_core::{ConceptId, ItemId};

use super::{FalkorGraph, id_list, read_rows};
use crate::contents::{Mention, Relation};
use crate::error::GraphError;

const RELATION_ROW: &str = "\
a relation (the id of the concept it leaves, the id of the concept it reaches, a kind of relation \
and the id of an item)";
const MENTION_ROW: &str = "a mention (an item id, a concept id and the wording)";

const RELATIONS_AMONG: &str = "\
MATCH (a:Concept)-[r:RELATES_TO]->(b:Concept)
WHERE a.id IN $concepts AND b.id IN $concepts
RETURN a.id, b.id, r.type, r.item
ORDER BY a.id, b.id, r.type";

const MENTIONS_BETWEEN: &str = "\
MATCH (i:Item)-[m:MENTIONS]->(c:Concept)
WHERE i.id IN $items AND c.id IN $concepts
RETURN i.id, c.id, m.wording
ORDER BY i.id, c.id";

impl FalkorGraph {
    /// Every `RELATES_TO` edge that has both ends among these concepts, with its direction (from
    /// the concept it leaves to the concept it reaches), its kind and the item that stated it. The
    /// edges are ordered by the id of the concept they leave, then by the id of the concept they
    /// reach, then by the name of their kind as the edge stores it. An empty slice makes no call.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a
    ///   relation
    pub async fn relations_among(
        &self,
        concepts: &[ConceptId],
    ) -> Result<Vec<Relation>, GraphError> {
        if concepts.is_empty() {
            return Ok(Vec::new());
        }
        let action = "read the relations among the concepts";
        let parameters = vec![("concepts", id_list(concepts))];
        let reply = self.run(action, RELATIONS_AMONG, parameters).await?;
        read_rows(self, action, RELATION_ROW, reply, relation_from_row)
    }

    /// Every `MENTIONS` edge from one of these items to one of these concepts, with its wording,
    /// ordered by the id of the item and then by the id of the concept. An empty slice, for the
    /// items or for the concepts, makes no call.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a
    ///   mention
    pub async fn mentions_between(
        &self,
        items: &[ItemId],
        concepts: &[ConceptId],
    ) -> Result<Vec<Mention>, GraphError> {
        if items.is_empty() || concepts.is_empty() {
            return Ok(Vec::new());
        }
        let action = "read the mentions of the concepts by the items";
        let parameters = vec![("items", id_list(items)), ("concepts", id_list(concepts))];
        let reply = self.run(action, MENTIONS_BETWEEN, parameters).await?;
        read_rows(self, action, MENTION_ROW, reply, mention_from_row)
    }
}

fn relation_from_row(row: Vec<FalkorValue>) -> Result<Relation, String> {
    let row = <[FalkorValue; 4]>::try_from(row).map_err(|row| format!("{row:?}"))?;
    let [
        FalkorValue::String(from),
        FalkorValue::String(to),
        FalkorValue::String(kind),
        FalkorValue::String(item),
    ] = row
    else {
        return Err(format!("{row:?}"));
    };
    Ok(Relation {
        from: from.parse().map_err(|_| format!("{from:?}"))?,
        to: to.parse().map_err(|_| format!("{to:?}"))?,
        kind: kind.parse().map_err(|_| format!("{kind:?}"))?,
        item: item.parse().map_err(|_| format!("{item:?}"))?,
    })
}

fn mention_from_row(row: Vec<FalkorValue>) -> Result<Mention, String> {
    let row = <[FalkorValue; 3]>::try_from(row).map_err(|row| format!("{row:?}"))?;
    let [
        FalkorValue::String(item),
        FalkorValue::String(concept),
        FalkorValue::String(wording),
    ] = row
    else {
        return Err(format!("{row:?}"));
    };
    Ok(Mention {
        item: item.parse().map_err(|_| format!("{item:?}"))?,
        concept: concept.parse().map_err(|_| format!("{concept:?}"))?,
        wording,
    })
}
