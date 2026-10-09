//! The indexes that let a statement find a node by its id or its name without reading every node
//! with its label.

use falkordb::FalkorValue;

use super::{FalkorGraph, unreadable_reply};
use crate::store::GraphError;

/// Each pair is a label and the property that statements find its nodes by. Media are few, so
/// their title has none.
const INDEXED: [(&str, &str); 5] = [
    ("Document", "id"),
    ("Item", "id"),
    ("Concept", "id"),
    ("Concept", "normalised_name"),
    // An index on a list finds a node by any one of the values in the list.
    ("Concept", "normalised_aliases"),
];

const INDEXES: &str = "CALL db.indexes() YIELD label, properties RETURN label, properties";

const INDEX_ROW: &str = "an index (a label and a list of properties)";

/// FalkorDB refuses a second index on the same property and has no form that skips one that
/// exists, so only the missing ones are made.
pub(super) async fn ensure(graph: &FalkorGraph) -> Result<(), GraphError> {
    let present = indexed(graph).await?;
    for (label, property) in INDEXED {
        if is_indexed(&present, label, property) {
            continue;
        }
        let statement = format!("CREATE INDEX FOR (n:{label}) ON (n.{property})");
        if let Err(error) = graph.run("create an index", &statement, Vec::new()).await {
            // Another program that connected at the same moment may have made it first.
            if !is_indexed(&indexed(graph).await?, label, property) {
                return Err(error);
            }
        }
    }
    Ok(())
}

fn is_indexed(present: &[(String, String)], label: &str, property: &str) -> bool {
    present.iter().any(|(indexed_label, indexed_property)| {
        indexed_label == label && indexed_property == property
    })
}

/// Every label and property that has an index.
async fn indexed(graph: &FalkorGraph) -> Result<Vec<(String, String)>, GraphError> {
    let action = "read the indexes";
    let reply = graph.run(action, INDEXES, Vec::new()).await?;
    let mut present = Vec::new();
    for row in reply.data.into_values_lossy() {
        let pairs = index_from_row(row)
            .map_err(|found| unreadable_reply(graph, action, INDEX_ROW, found))?;
        present.extend(pairs);
    }
    Ok(present)
}

fn index_from_row(row: Vec<FalkorValue>) -> Result<Vec<(String, String)>, String> {
    let row = <[FalkorValue; 2]>::try_from(row).map_err(|row| format!("{row:?}"))?;
    let [FalkorValue::String(label), FalkorValue::Array(properties)] = row else {
        return Err(format!("{row:?}"));
    };
    properties
        .into_iter()
        .map(|property| match property {
            FalkorValue::String(property) => Ok((label.clone(), property)),
            other => Err(format!("{other:?}")),
        })
        .collect()
}
