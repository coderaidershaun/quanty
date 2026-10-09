//! The statements that read concepts and the items that mention them, with the code that runs
//! each one and the row it expects.

use falkordb::FalkorValue;
use rag_core::{ConceptId, DocId, ItemId};

use super::{FalkorGraph, id_list, id_value, unreadable_reply};
use crate::contents::{ConceptNode, ItemMentions};
use crate::store::GraphError;

const CONCEPT_ROW: &str = "a concept (an id, a name, a normalised name and a definition)";
const ITEM_ROW: &str =
    "an item with the concepts it mentions (an item id and a list of concept ids)";

const FIND_CONCEPT: &str = "\
MATCH (c:Concept)
WHERE c.normalised_name = $name OR $name IN c.normalised_aliases
RETURN c.id, c.name, c.normalised_name, c.definition
LIMIT 1";

const CONCEPT_BY_ID: &str = "\
MATCH (c:Concept {id: $id})
RETURN c.id, c.name, c.normalised_name, c.definition
LIMIT 1";

const CONCEPTS_FOR_ITEMS: &str = "\
MATCH (i:Item)-[:MENTIONS]->(c:Concept)
WHERE i.id IN $items
WITH c, count(i) AS mentions
RETURN c.id, c.name, c.normalised_name, c.definition
ORDER BY mentions DESC, c.normalised_name";

const ITEMS_FOR_CONCEPTS: &str = "\
MATCH (i:Item)-[:MENTIONS]->(c:Concept)
WHERE c.id IN $concepts
WITH i, collect(c.id) AS shared
RETURN i.id, shared
ORDER BY size(shared) DESC, i.id
LIMIT $limit";

const RELATED_CONCEPTS: &str = "\
MATCH (a:Concept)-[:RELATES_TO]-(b:Concept)
WHERE a.id IN $concepts AND NOT b.id IN $concepts
RETURN DISTINCT b.id, b.name, b.normalised_name, b.definition
ORDER BY b.normalised_name";

const CONCEPTS_ON_PAGE: &str = "\
MATCH (d:Document {id: $document})-[:HAS_ITEM]->(i:Item)-[:MENTIONS]->(c:Concept)
WHERE i.page = $page
WITH c, count(i) AS mentions
RETURN c.id, c.name, c.normalised_name, c.definition
ORDER BY mentions DESC, c.normalised_name";

pub(super) async fn find_concept_by_name(
    graph: &FalkorGraph,
    normalised_name: &str,
) -> Result<Option<ConceptNode>, GraphError> {
    let parameters = vec![("name", FalkorValue::String(normalised_name.to_owned()))];
    read_concept(
        graph,
        "look up a concept by its name",
        FIND_CONCEPT,
        parameters,
    )
    .await
}

pub(super) async fn concept(
    graph: &FalkorGraph,
    id: ConceptId,
) -> Result<Option<ConceptNode>, GraphError> {
    let parameters = vec![("id", id_value(id))];
    read_concept(graph, "read a concept by its id", CONCEPT_BY_ID, parameters).await
}

async fn read_concept(
    graph: &FalkorGraph,
    action: &'static str,
    statement: &str,
    parameters: Vec<(&'static str, FalkorValue)>,
) -> Result<Option<ConceptNode>, GraphError> {
    let concepts = read_concepts(graph, action, statement, parameters).await?;
    Ok(concepts.into_iter().next())
}

async fn read_concepts(
    graph: &FalkorGraph,
    action: &'static str,
    statement: &str,
    parameters: Vec<(&'static str, FalkorValue)>,
) -> Result<Vec<ConceptNode>, GraphError> {
    let reply = graph.run(action, statement, parameters).await?;
    reply
        .data
        .into_values_lossy()
        .map(concept_from_row)
        .collect::<Result<_, _>>()
        .map_err(|found| unreadable_reply(graph, action, CONCEPT_ROW, found))
}

pub(super) async fn concepts_for_items(
    graph: &FalkorGraph,
    items: &[ItemId],
) -> Result<Vec<ConceptNode>, GraphError> {
    if items.is_empty() {
        return Ok(Vec::new());
    }
    let parameters = vec![("items", id_list(items))];
    read_concepts(
        graph,
        "read the concepts that the items mention",
        CONCEPTS_FOR_ITEMS,
        parameters,
    )
    .await
}

pub(super) async fn items_for_concepts(
    graph: &FalkorGraph,
    concepts: &[ConceptId],
    limit: usize,
) -> Result<Vec<ItemMentions>, GraphError> {
    if concepts.is_empty() {
        return Ok(Vec::new());
    }
    let action = "read the items that mention the concepts";
    let parameters = vec![
        ("concepts", id_list(concepts)),
        (
            "limit",
            FalkorValue::I64(i64::try_from(limit).unwrap_or(i64::MAX)),
        ),
    ];
    let reply = graph.run(action, ITEMS_FOR_CONCEPTS, parameters).await?;
    reply
        .data
        .into_values_lossy()
        .map(item_mentions_from_row)
        .collect::<Result<_, _>>()
        .map_err(|found| unreadable_reply(graph, action, ITEM_ROW, found))
}

pub(super) async fn related_concepts(
    graph: &FalkorGraph,
    concepts: &[ConceptId],
) -> Result<Vec<ConceptNode>, GraphError> {
    if concepts.is_empty() {
        return Ok(Vec::new());
    }
    let parameters = vec![("concepts", id_list(concepts))];
    read_concepts(
        graph,
        "read the concepts related to the concepts",
        RELATED_CONCEPTS,
        parameters,
    )
    .await
}

pub(super) async fn concepts_on_page(
    graph: &FalkorGraph,
    document: DocId,
    page: u32,
) -> Result<Vec<ConceptNode>, GraphError> {
    let parameters = vec![
        ("document", id_value(document)),
        ("page", FalkorValue::I64(i64::from(page))),
    ];
    read_concepts(
        graph,
        "read the concepts that the items on a page mention",
        CONCEPTS_ON_PAGE,
        parameters,
    )
    .await
}

fn concept_from_row(row: Vec<FalkorValue>) -> Result<ConceptNode, String> {
    let row = <[FalkorValue; 4]>::try_from(row).map_err(|row| format!("{row:?}"))?;
    let [
        FalkorValue::String(id),
        FalkorValue::String(name),
        FalkorValue::String(normalised_name),
        FalkorValue::String(definition),
    ] = row
    else {
        return Err(format!("{row:?}"));
    };
    let id = id.parse().map_err(|_| format!("{id:?}"))?;
    Ok(ConceptNode {
        id,
        name,
        normalised_name,
        definition,
    })
}

fn item_mentions_from_row(row: Vec<FalkorValue>) -> Result<ItemMentions, String> {
    let row = <[FalkorValue; 2]>::try_from(row).map_err(|row| format!("{row:?}"))?;
    let [FalkorValue::String(item), FalkorValue::Array(concepts)] = row else {
        return Err(format!("{row:?}"));
    };
    let item = item.parse().map_err(|_| format!("{item:?}"))?;
    let concepts = concepts
        .into_iter()
        .map(|concept| match concept {
            FalkorValue::String(id) => id.parse().map_err(|_| format!("{id:?}")),
            other => Err(format!("{other:?}")),
        })
        .collect::<Result<_, _>>()?;
    Ok(ItemMentions { item, concepts })
}
