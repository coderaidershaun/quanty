//! The statements that read documents, saved books, concepts, the items that mention them, and
//! whether a document is ingested whole, with the code that runs each one and the row it expects.

use std::collections::BTreeSet;

use falkordb::FalkorValue;
use rag_core::{ConceptId, DocId, DocumentLabels, ItemId, Tag};

use super::{FalkorGraph, id_value};
use crate::contents::{BookNode, ConceptNode, DocumentNode, ItemMentions};
use crate::store::GraphError;

const DOCUMENT_ROW: &str =
    "a document (an id, a title, a book or null, an author or null and a list of tags or null)";
const BOOK_ROW: &str = "a book (a title, an author or null and a list of tags or null)";
const CONCEPT_ROW: &str = "a concept (an id, a name, a normalised name and a definition)";
const ITEM_ROW: &str =
    "an item with the concepts it mentions (an item id and a list of concept ids)";
const INGESTED_ITEMS_ROW: &str = "a count of items or null (one whole number that is not negative)";

/// What a statement returns for a document `d`, in the order that `document_from_row` reads it.
/// Every statement that reads a document takes its columns from here.
pub(super) const DOCUMENT_COLUMNS: &str = "d.id, d.title, d.book, d.author, d.tags";

// SMELL: there is no index on the normalised name either, and an index could not cover the list
// of aliases, so each lookup reads every concept node.
const FIND_CONCEPT: &str = "\
MATCH (c:Concept)
WHERE c.normalised_name = $name OR $name IN c.normalised_aliases
RETURN c.id, c.name, c.normalised_name, c.definition
LIMIT 1";

const CONCEPT_BY_ID: &str = "\
MATCH (c:Concept {id: $id})
RETURN c.id, c.name, c.normalised_name, c.definition
LIMIT 1";

// SMELL: this reads every document, and it is read by each ingest, each `tag` and each query that
// has a label to match. Four documents are nothing, and a large library would need a read by id
// and a read of the ids that carry given labels.
fn documents_statement() -> String {
    format!(
        "\
MATCH (d:Document)
RETURN {DOCUMENT_COLUMNS}
ORDER BY d.id"
    )
}

const BOOKS: &str = "\
MATCH (b:Book)
RETURN b.title, b.author, b.tags
ORDER BY b.title";

const INGESTED_ITEMS: &str = "\
MATCH (d:Document {id: $id})
RETURN d.ingested_items
LIMIT 1";

// SMELL: a read of a graph that does not exist is not an error. FalkorDB answers with no rows and
// creates an empty graph of that name, so a program that reads before anything was written finds
// nothing and leaves an empty graph behind. The read-only form of the query fails instead, and
// this code does not use it.
// SMELL: like the writes, the four reads below have no index on `id`, so each one looks at every
// node with its label and gets slower as the graph grows.
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

pub(super) async fn documents(graph: &FalkorGraph) -> Result<Vec<DocumentNode>, GraphError> {
    let action = "read the documents";
    let reply = graph
        .run(action, &documents_statement(), Vec::new())
        .await?;
    reply
        .data
        .into_values_lossy()
        .map(document_from_row)
        .collect::<Result<_, _>>()
        .map_err(|found| unreadable_reply(graph, action, DOCUMENT_ROW, found))
}

pub(super) async fn books(graph: &FalkorGraph) -> Result<Vec<BookNode>, GraphError> {
    let action = "read the saved books";
    let reply = graph.run(action, BOOKS, Vec::new()).await?;
    reply
        .data
        .into_values_lossy()
        .map(book_from_row)
        .collect::<Result<_, _>>()
        .map_err(|found| unreadable_reply(graph, action, BOOK_ROW, found))
}

pub(super) async fn ingested_items(
    graph: &FalkorGraph,
    document: DocId,
) -> Result<Option<u64>, GraphError> {
    let action = "read how many items the document was ingested with";
    let parameters = vec![("id", id_value(document))];
    let reply = graph.run(action, INGESTED_ITEMS, parameters).await?;
    match reply.data.into_values_lossy().next() {
        None => Ok(None),
        Some(row) => ingested_items_from_row(row)
            .map_err(|found| unreadable_reply(graph, action, INGESTED_ITEMS_ROW, found)),
    }
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

pub(super) fn unreadable_reply(
    graph: &FalkorGraph,
    action: &'static str,
    expected: &'static str,
    found: String,
) -> GraphError {
    GraphError::UnreadableReply {
        url: graph.url.clone(),
        graph: graph.graph.graph_name().to_owned(),
        action,
        found,
        expected,
    }
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

pub(super) fn id_list(ids: &[impl ToString + Copy]) -> FalkorValue {
    FalkorValue::Array(ids.iter().copied().map(id_value).collect())
}

pub(super) fn document_from_row(row: Vec<FalkorValue>) -> Result<DocumentNode, String> {
    let row = <[FalkorValue; 5]>::try_from(row).map_err(|row| format!("{row:?}"))?;
    let [
        FalkorValue::String(id),
        FalkorValue::String(title),
        book,
        author,
        tags,
    ] = row
    else {
        return Err(format!("{row:?}"));
    };
    let tags = tag_set(tags)?;
    Ok(DocumentNode {
        id: id.parse().map_err(|_| format!("{id:?}"))?,
        title,
        labels: DocumentLabels {
            book: text_or_null(book)?,
            author: text_or_null(author)?,
            tags,
        },
    })
}

fn book_from_row(row: Vec<FalkorValue>) -> Result<BookNode, String> {
    let row = <[FalkorValue; 3]>::try_from(row).map_err(|row| format!("{row:?}"))?;
    let [FalkorValue::String(title), author, tags] = row else {
        return Err(format!("{row:?}"));
    };
    Ok(BookNode {
        title,
        author: text_or_null(author)?,
        tags: tag_set(tags)?,
    })
}

fn text_or_null(value: FalkorValue) -> Result<Option<String>, String> {
    match value {
        FalkorValue::None => Ok(None),
        FalkorValue::String(text) => Ok(Some(text)),
        other => Err(format!("{other:?}")),
    }
}

fn tag_set(value: FalkorValue) -> Result<BTreeSet<Tag>, String> {
    let tags = match value {
        FalkorValue::None => return Ok(BTreeSet::new()),
        FalkorValue::Array(tags) => tags,
        other => return Err(format!("{other:?}")),
    };
    tags.into_iter()
        .map(|tag| match tag {
            FalkorValue::String(text) => text.parse::<Tag>().map_err(|_| format!("{text:?}")),
            other => Err(format!("{other:?}")),
        })
        .collect()
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

fn ingested_items_from_row(row: Vec<FalkorValue>) -> Result<Option<u64>, String> {
    let row = <[FalkorValue; 1]>::try_from(row).map_err(|row| format!("{row:?}"))?;
    match row {
        [FalkorValue::None] => Ok(None),
        [FalkorValue::I64(count)] => u64::try_from(count)
            .map(Some)
            .map_err(|_| count.to_string()),
        other => Err(format!("{other:?}")),
    }
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
