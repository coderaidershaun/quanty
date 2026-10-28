//! The statements that follow `RELATES_TO` edges from concepts: the edges that touch some
//! concepts, and the shortest chain of edges between two concepts.

use falkordb::FalkorValue;
use rag_core::ConceptId;

use super::edges::{RELATION_ROW, relation_from_row};
use super::mentions::count_value;
use super::{FalkorGraph, fixed, id_list, id_value, read_rows, unreadable_reply};
use crate::contents::{Direction, PathDepth, Relation, RelationKind};
use crate::error::GraphError;

const PATH_ROW: &str = "\
a list of relations, each a list of the id of the concept it leaves, the id of the concept it \
reaches, a kind of relation and the id of an item";

// The three statements differ only in the end that must be one of the concepts. An empty list of
// kinds lets every kind through.
const NEIGHBOURS_OUT: &str = "\
MATCH (a:Concept)-[r:RELATES_TO]->(b:Concept)
WHERE a.id IN $concepts AND (size($kinds) = 0 OR r.type IN $kinds)
RETURN a.id, b.id, r.type, r.item
ORDER BY a.id, b.id, r.type
LIMIT $limit";

const NEIGHBOURS_IN: &str = "\
MATCH (a:Concept)-[r:RELATES_TO]->(b:Concept)
WHERE b.id IN $concepts AND (size($kinds) = 0 OR r.type IN $kinds)
RETURN a.id, b.id, r.type, r.item
ORDER BY a.id, b.id, r.type
LIMIT $limit";

const NEIGHBOURS_BOTH: &str = "\
MATCH (a:Concept)-[r:RELATES_TO]->(b:Concept)
WHERE (a.id IN $concepts OR b.id IN $concepts) AND (size($kinds) = 0 OR r.type IN $kinds)
RETURN a.id, b.id, r.type, r.item
ORDER BY a.id, b.id, r.type
LIMIT $limit";

// FalkorDB takes `shortestPath` only in a `WITH` or a `RETURN`, and the bound of a variable-length
// pattern cannot be a parameter. So the bound is written as 4, which must stay equal to
// `PathDepth::MAX`, and the `WHERE` drops a path longer than the caller asked for.
const SHORTEST_PATH: &str = "\
MATCH (a:Concept {id: $from}), (b:Concept {id: $to})
WITH shortestPath((a)-[:RELATES_TO*1..4]-(b)) AS path
WHERE path IS NOT NULL AND length(path) <= $max_depth
RETURN [r IN relationships(path) | [startNode(r).id, endNode(r).id, r.type, r.item]]";

impl FalkorGraph {
    /// The `RELATES_TO` edges that touch one of the concepts, one hop from them. `Out` keeps the
    /// edges that leave one of the concepts, `In` the edges that reach one, and `Both` either.
    /// Only the given kinds are kept, and an empty slice of kinds keeps every kind. Each edge comes
    /// once, with the direction and the item it was written with, ordered by the id of the
    /// concept it leaves, then by the id of the concept it reaches, then by the name of its kind
    /// as the edge stores it. At most `limit` come back. An empty slice of concepts makes no call.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a
    ///   relation
    pub async fn neighbours(
        &self,
        concepts: &[ConceptId],
        kinds: &[RelationKind],
        direction: Direction,
        limit: usize,
    ) -> Result<Vec<Relation>, GraphError> {
        if concepts.is_empty() {
            return Ok(Vec::new());
        }
        let action = "read the relations that touch the concepts";
        let statement = match direction {
            Direction::Out => NEIGHBOURS_OUT,
            Direction::In => NEIGHBOURS_IN,
            Direction::Both => NEIGHBOURS_BOTH,
        };
        let kinds = kinds
            .iter()
            .map(|kind| FalkorValue::String(kind.as_str().to_owned()))
            .collect();
        let parameters = vec![
            ("concepts", id_list(concepts)),
            ("kinds", FalkorValue::Array(kinds)),
            ("limit", count_value(limit)),
        ];
        let reply = self.run(action, statement, parameters).await?;
        read_rows(self, action, RELATION_ROW, reply, relation_from_row)
    }

    /// One shortest chain of `RELATES_TO` edges from one concept to the other, followed in either
    /// direction, of at most `max_depth` edges. The edges come in the order of the chain, from
    /// `from` to `to`, each with the direction and the item it was written with. `None` when
    /// there is no chain that short. When two chains are equally short, which one comes back is
    /// not fixed. `from` and `to` are two different concepts.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a list
    ///   of relations
    pub async fn shortest_path(
        &self,
        from: ConceptId,
        to: ConceptId,
        max_depth: PathDepth,
    ) -> Result<Option<Vec<Relation>>, GraphError> {
        let action = "read the shortest chain of relations between two concepts";
        let parameters = vec![
            ("from", id_value(from)),
            ("to", id_value(to)),
            ("max_depth", FalkorValue::I64(i64::from(max_depth.edges()))),
        ];
        let reply = self.run(action, SHORTEST_PATH, parameters).await?;
        reply
            .data
            .into_values_lossy()
            .next()
            .map(path_from_row)
            .transpose()
            .map_err(|found| unreadable_reply(self, action, PATH_ROW, found))
    }
}

fn path_from_row(row: Vec<FalkorValue>) -> Result<Vec<Relation>, String> {
    let [FalkorValue::Array(edges)] = fixed::<1>(row)? else {
        return Err("a value that is not a list".to_owned());
    };
    edges
        .into_iter()
        .map(|edge| match edge {
            FalkorValue::Array(edge) => relation_from_row(edge),
            other => Err(format!("{other:?}")),
        })
        .collect()
}
