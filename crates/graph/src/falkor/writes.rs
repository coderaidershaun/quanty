//! The statements that write documents, items, concepts, mentions and relations, with the code
//! that runs each one and the rows it takes.

use std::collections::HashMap;
use std::path::Path;

use falkordb::FalkorValue;
use rag_core::DocId;

use super::{FalkorGraph, id_value};
use crate::contents::{ConceptAlias, ConceptNode, DocumentNode, ItemNode, Mention, Relation};
use crate::store::GraphError;

/// Rows go in groups of this size, so that one statement stays quick also when the graph is
/// large: the client waits only a short time for a reply.
const ROWS_PER_STATEMENT: usize = 200;

// FalkorDB removes a property that is set to null, so a label that the document does not have is
// taken away.
const UPSERT_DOCUMENT: &str = "\
MERGE (d:Document {id: $id})
SET d.title = $title, d.book = $book, d.author = $author, d.tags = $tags";

const SET_INGESTED_ITEMS: &str = "MATCH (d:Document {id: $id}) SET d.ingested_items = $items";

const SET_CHAPTER_FOLDER: &str = "MATCH (d:Document {id: $id}) SET d.chapter_folder = $folder";

// SMELL: there is no index on `id`, so every `MERGE` reads all the nodes with its label, and a
// write gets slower as the graph grows. In a very large graph one write would take longer than
// the client waits for a reply, and the ingest would stop with a failed request.
const UPSERT_ITEMS: &str = "\
MERGE (d:Document {id: $document})
WITH d
UNWIND $items AS item
MERGE (i:Item {id: item.id})
SET i.kind = item.kind, i.page = item.page, i.printed_page = item.printed_page
MERGE (d)-[:HAS_ITEM]->(i)";

const LINK_NEXT: &str = "\
UNWIND $pairs AS pair
MATCH (a:Item {id: pair[0]}), (b:Item {id: pair[1]})
MERGE (a)-[:NEXT]->(b)";

const DELETE_DOCUMENT: &str = "\
MATCH (d:Document {id: $id})
OPTIONAL MATCH (d)-[:HAS_ITEM]->(i:Item)
DETACH DELETE i, d";

// A concept keeps each alias twice: as the item wrote it, for a person to read, and in the form
// that names are compared in, for the lookup. The two lists must grow together.
const UPSERT_CONCEPT: &str = "\
MERGE (c:Concept {id: $id})
ON CREATE SET c.aliases = [], c.normalised_aliases = []
SET c.name = $name, c.normalised_name = $normalised_name, c.definition = $definition";

const ADD_ALIAS: &str = "\
MATCH (c:Concept {id: $id})
WHERE c.normalised_name <> $normalised_name
  AND NOT $normalised_name IN coalesce(c.normalised_aliases, [])
SET c.aliases = coalesce(c.aliases, []) + $name,
    c.normalised_aliases = coalesce(c.normalised_aliases, []) + $normalised_name";

// SMELL: a mention whose item or concept is not in the graph is not written, and nothing reports
// it.
const ADD_MENTIONS: &str = "\
UNWIND $mentions AS mention
MATCH (i:Item {id: mention.item}), (c:Concept {id: mention.concept})
MERGE (i)-[m:MENTIONS]->(c)
SET m.wording = mention.wording";

// SMELL: a relation whose concepts are not in the graph is not written, and nothing reports it.
// A relation also keeps the id of the item that stated it after the document of that item is
// deleted, and a concept stays when no item mentions it any more.
const ADD_RELATIONS: &str = "\
UNWIND $relations AS relation
MATCH (a:Concept {id: relation.from}), (b:Concept {id: relation.to})
MERGE (a)-[r:RELATES_TO {type: relation.type}]->(b)
ON CREATE SET r.item = relation.item";

pub(super) async fn upsert_document(
    graph: &FalkorGraph,
    document: &DocumentNode,
) -> Result<(), GraphError> {
    let labels = &document.labels;
    let text_or_null = |text: &Option<String>| match text {
        Some(text) => FalkorValue::String(text.clone()),
        None => FalkorValue::None,
    };
    let tags = labels
        .tags
        .iter()
        .map(|tag| FalkorValue::String(tag.to_string()))
        .collect();
    let parameters = vec![
        ("id", id_value(document.id)),
        ("title", FalkorValue::String(document.title.clone())),
        ("book", text_or_null(&labels.book)),
        ("author", text_or_null(&labels.author)),
        ("tags", FalkorValue::Array(tags)),
    ];
    graph
        .run("write the document", UPSERT_DOCUMENT, parameters)
        .await?;
    Ok(())
}

pub(super) async fn set_ingested_items(
    graph: &FalkorGraph,
    document: DocId,
    items: Option<u64>,
) -> Result<(), GraphError> {
    // FalkorDB removes a property that is set to null.
    let items = match items {
        Some(count) => FalkorValue::I64(i64::try_from(count).unwrap_or(i64::MAX)),
        None => FalkorValue::None,
    };
    let parameters = vec![("id", id_value(document)), ("items", items)];
    graph
        .run(
            "record whether the document is ingested whole",
            SET_INGESTED_ITEMS,
            parameters,
        )
        .await?;
    Ok(())
}

pub(super) async fn set_chapter_folder(
    graph: &FalkorGraph,
    document: DocId,
    folder: &Path,
) -> Result<(), GraphError> {
    let text = folder
        .to_str()
        .ok_or_else(|| GraphError::FolderNotUnicode {
            folder: folder.to_path_buf(),
        })?;
    let parameters = vec![
        ("id", id_value(document)),
        ("folder", FalkorValue::String(text.to_owned())),
    ];
    graph
        .run(
            "record the folder the document was ingested from",
            SET_CHAPTER_FOLDER,
            parameters,
        )
        .await?;
    Ok(())
}

pub(super) async fn upsert_items(
    graph: &FalkorGraph,
    document: DocId,
    items: &[ItemNode],
) -> Result<(), GraphError> {
    for group in items.chunks(ROWS_PER_STATEMENT) {
        let rows = group.iter().map(item_row).collect();
        let parameters = vec![
            ("document", id_value(document)),
            ("items", FalkorValue::Array(rows)),
        ];
        graph
            .run("write the items", UPSERT_ITEMS, parameters)
            .await?;
    }
    // The pairs come from the whole slice, so a pair that sits across two groups of nodes is
    // not lost. The nodes are all written first, so every pair finds both of its ends.
    let pairs: Vec<FalkorValue> = items
        .windows(2)
        .map(|pair| FalkorValue::Array(vec![id_value(pair[0].id), id_value(pair[1].id)]))
        .collect();
    for group in pairs.chunks(ROWS_PER_STATEMENT) {
        let parameters = vec![("pairs", FalkorValue::Array(group.to_vec()))];
        graph
            .run("write the order of the items", LINK_NEXT, parameters)
            .await?;
    }
    Ok(())
}

pub(super) async fn delete_document(graph: &FalkorGraph, id: DocId) -> Result<u64, GraphError> {
    let reply = graph
        .run(
            "delete the document",
            DELETE_DOCUMENT,
            vec![("id", id_value(id))],
        )
        .await?;
    // FalkorDB leaves the count of deleted nodes out of its reply when it deleted none.
    // SMELL: the client also gives no count when it cannot find or read that line of the
    // reply, so if FalkorDB changes the wording, a delete that removed nodes is reported as
    // zero.
    let removed = reply
        .get_nodes_deleted()
        .and_then(|count| u64::try_from(count).ok());
    Ok(removed.unwrap_or(0))
}

pub(super) async fn upsert_concept(
    graph: &FalkorGraph,
    concept: &ConceptNode,
) -> Result<(), GraphError> {
    let parameters = vec![
        ("id", id_value(concept.id)),
        ("name", FalkorValue::String(concept.name.clone())),
        (
            "normalised_name",
            FalkorValue::String(concept.normalised_name.clone()),
        ),
        (
            "definition",
            FalkorValue::String(concept.definition.clone()),
        ),
    ];
    graph
        .run("write the concept", UPSERT_CONCEPT, parameters)
        .await?;
    Ok(())
}

pub(super) async fn add_mentions(
    graph: &FalkorGraph,
    mentions: &[Mention],
) -> Result<(), GraphError> {
    for group in mentions.chunks(ROWS_PER_STATEMENT) {
        let rows = group.iter().map(mention_row).collect();
        let parameters = vec![("mentions", FalkorValue::Array(rows))];
        graph
            .run("write the mentions", ADD_MENTIONS, parameters)
            .await?;
    }
    Ok(())
}

pub(super) async fn add_relations(
    graph: &FalkorGraph,
    relations: &[Relation],
) -> Result<(), GraphError> {
    for group in relations.chunks(ROWS_PER_STATEMENT) {
        let rows = group.iter().map(relation_row).collect();
        let parameters = vec![("relations", FalkorValue::Array(rows))];
        graph
            .run("write the relations", ADD_RELATIONS, parameters)
            .await?;
    }
    Ok(())
}

pub(super) async fn add_alias(graph: &FalkorGraph, alias: &ConceptAlias) -> Result<(), GraphError> {
    let parameters = vec![
        ("id", id_value(alias.concept)),
        ("name", FalkorValue::String(alias.name.clone())),
        (
            "normalised_name",
            FalkorValue::String(alias.normalised_name.clone()),
        ),
    ];
    graph
        .run("add an alias to the concept", ADD_ALIAS, parameters)
        .await?;
    Ok(())
}

fn item_row(item: &ItemNode) -> FalkorValue {
    let printed_page = match &item.printed_page {
        Some(printed_page) => FalkorValue::String(printed_page.clone()),
        None => FalkorValue::None,
    };
    FalkorValue::Map(HashMap::from([
        ("id".to_owned(), id_value(item.id)),
        (
            "kind".to_owned(),
            FalkorValue::String(item.kind.as_str().to_owned()),
        ),
        ("page".to_owned(), FalkorValue::I64(i64::from(item.page))),
        ("printed_page".to_owned(), printed_page),
    ]))
}

fn mention_row(mention: &Mention) -> FalkorValue {
    FalkorValue::Map(HashMap::from([
        ("item".to_owned(), id_value(mention.item)),
        ("concept".to_owned(), id_value(mention.concept)),
        (
            "wording".to_owned(),
            FalkorValue::String(mention.wording.clone()),
        ),
    ]))
}

fn relation_row(relation: &Relation) -> FalkorValue {
    FalkorValue::Map(HashMap::from([
        ("from".to_owned(), id_value(relation.from)),
        ("to".to_owned(), id_value(relation.to)),
        (
            "type".to_owned(),
            FalkorValue::String(relation.kind.as_str().to_owned()),
        ),
        ("item".to_owned(), id_value(relation.item)),
    ]))
}
