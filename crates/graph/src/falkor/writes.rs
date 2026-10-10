//! The statements that write documents, media, items, concepts, mentions and relations,
//! with the code that runs each one and the rows it takes.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use falkordb::FalkorValue;
use rag_core::{DocId, Tag};

use super::{FalkorGraph, id_value, unreadable_reply};
use crate::contents::{
    ConceptAlias, ConceptNode, DocumentNode, ItemNode, MediaNode, Mention, Relation,
};
use crate::store::GraphError;

/// Rows go in groups of this size, so that one statement stays quick also when the graph is
/// large: the client waits only a short time for a reply.
const ROWS_PER_STATEMENT: usize = 200;

// FalkorDB removes a property that is set to null, so a label that the document does not have is
// taken away.
const UPSERT_DOCUMENT: &str = "\
MERGE (d:Document {id: $id})
SET d.title = $title, d.media = $media, d.category = $category, d.authors = $authors,
    d.media_tags = $media_tags, d.tags = $tags";

// Only a new media gets its labels, so a second add of a title never writes over the media that is
// stored.
const ADD_MEDIA: &str = "\
MERGE (m:Media {title: $title})
ON CREATE SET m.category = $category, m.authors = $authors, m.tags = $tags";

const UPDATE_MEDIA: &str = "\
MERGE (m:Media {title: $title})
SET m.category = $category, m.authors = $authors, m.tags = $tags";

// The statement counts what it deletes itself, because the line of the reply that counts deleted
// nodes is left out when there are none.
const DELETE_MEDIA: &str = "\
MATCH (m:Media {title: $title})
DETACH DELETE m
RETURN count(m)";

const SET_INGESTED_ITEMS: &str = "MATCH (d:Document {id: $id}) SET d.ingested_items = $items";

const SET_CHAPTER_FOLDER: &str = "MATCH (d:Document {id: $id}) SET d.chapter_folder = $folder";

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

// The statement counts what it deletes itself, because the line of the reply that counts deleted
// nodes is left out when there are none, and the client cannot tell that from a line it failed
// to read.
const DELETE_DOCUMENT: &str = "\
MATCH (d:Document {id: $id})
OPTIONAL MATCH (d)-[:HAS_ITEM]->(i:Item)
DETACH DELETE i, d
RETURN count(DISTINCT d) + count(DISTINCT i)";

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

// A row whose item or concept is not in the graph matches nothing and is dropped without an error,
// so the statement counts the rows it wrote.
const ADD_MENTIONS: &str = "\
UNWIND $mentions AS mention
MATCH (i:Item {id: mention.item}), (c:Concept {id: mention.concept})
MERGE (i)-[m:MENTIONS]->(c)
SET m.wording = mention.wording
RETURN count(m)";

// SMELL: a relation keeps the id of the item that stated it after the document of that item is
// deleted, and a concept stays when no item mentions it any more.
const ADD_RELATIONS: &str = "\
UNWIND $relations AS relation
MATCH (a:Concept {id: relation.from}), (b:Concept {id: relation.to})
MERGE (a)-[r:RELATES_TO {type: relation.type}]->(b)
ON CREATE SET r.item = relation.item
RETURN count(r)";

const COUNT_ROW: &str = "a count (one whole number that is not negative)";

pub(super) async fn upsert_document(
    graph: &FalkorGraph,
    document: &DocumentNode,
) -> Result<(), GraphError> {
    let labels = &document.labels;
    let parameters = vec![
        ("id", id_value(document.id)),
        ("title", FalkorValue::String(document.title.clone())),
        ("media", text_or_null(labels.media.as_deref())),
        (
            "category",
            text_or_null(labels.category.map(|category| category.as_str())),
        ),
        ("authors", text_list(&labels.authors)),
        ("media_tags", tag_list(&labels.media_tags)),
        ("tags", tag_list(&labels.tags)),
    ];
    graph
        .run("write the document", UPSERT_DOCUMENT, parameters)
        .await?;
    Ok(())
}

pub(super) async fn add_media(graph: &FalkorGraph, media: &MediaNode) -> Result<(), GraphError> {
    graph
        .run("write the media", ADD_MEDIA, media_parameters(media))
        .await?;
    Ok(())
}

pub(super) async fn update_media(graph: &FalkorGraph, media: &MediaNode) -> Result<(), GraphError> {
    graph
        .run(
            "write the labels of the media",
            UPDATE_MEDIA,
            media_parameters(media),
        )
        .await?;
    Ok(())
}

fn media_parameters(media: &MediaNode) -> Vec<(&'static str, FalkorValue)> {
    vec![
        ("title", FalkorValue::String(media.title.clone())),
        (
            "category",
            FalkorValue::String(media.labels.category.as_str().to_owned()),
        ),
        ("authors", text_list(&media.labels.authors)),
        ("tags", tag_list(&media.labels.tags)),
    ]
}

pub(super) async fn delete_media(graph: &FalkorGraph, title: &str) -> Result<bool, GraphError> {
    let parameters = vec![("title", FalkorValue::String(title.to_owned()))];
    let removed = run_counted(graph, "delete the media", DELETE_MEDIA, parameters).await?;
    Ok(removed > 0)
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
    let parameters = vec![("id", id_value(id))];
    run_counted(graph, "delete the document", DELETE_DOCUMENT, parameters).await
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
    let mut written = 0;
    for group in mentions.chunks(ROWS_PER_STATEMENT) {
        let rows = group.iter().map(mention_row).collect();
        let parameters = vec![("mentions", FalkorValue::Array(rows))];
        written += run_counted(graph, "write the mentions", ADD_MENTIONS, parameters).await?;
    }
    all_written(graph, "mentions", mentions.len(), written)
}

pub(super) async fn add_relations(
    graph: &FalkorGraph,
    relations: &[Relation],
) -> Result<(), GraphError> {
    let mut written = 0;
    for group in relations.chunks(ROWS_PER_STATEMENT) {
        let rows = group.iter().map(relation_row).collect();
        let parameters = vec![("relations", FalkorValue::Array(rows))];
        written += run_counted(graph, "write the relations", ADD_RELATIONS, parameters).await?;
    }
    all_written(graph, "relations", relations.len(), written)
}

fn all_written(
    graph: &FalkorGraph,
    edges: &'static str,
    asked: usize,
    written: u64,
) -> Result<(), GraphError> {
    let asked = asked as u64;
    if written == asked {
        return Ok(());
    }
    Err(GraphError::MissingNodes {
        url: graph.url.clone(),
        graph: graph.graph.graph_name().to_owned(),
        edges,
        asked,
        written,
    })
}

async fn run_counted(
    graph: &FalkorGraph,
    action: &'static str,
    statement: &str,
    parameters: Vec<(&'static str, FalkorValue)>,
) -> Result<u64, GraphError> {
    let reply = graph.run(action, statement, parameters).await?;
    let rows: Vec<Vec<FalkorValue>> = reply.data.into_values_lossy().collect();
    count_from_rows(&rows).map_err(|found| unreadable_reply(graph, action, COUNT_ROW, found))
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

fn text_or_null(text: Option<&str>) -> FalkorValue {
    match text {
        Some(text) => FalkorValue::String(text.to_owned()),
        None => FalkorValue::None,
    }
}

fn text_list(texts: &[String]) -> FalkorValue {
    FalkorValue::Array(texts.iter().cloned().map(FalkorValue::String).collect())
}

fn tag_list(tags: &BTreeSet<Tag>) -> FalkorValue {
    FalkorValue::Array(
        tags.iter()
            .map(|tag| FalkorValue::String(tag.to_string()))
            .collect(),
    )
}

fn item_row(item: &ItemNode) -> FalkorValue {
    FalkorValue::Map(HashMap::from([
        ("id".to_owned(), id_value(item.id)),
        (
            "kind".to_owned(),
            FalkorValue::String(item.kind.as_str().to_owned()),
        ),
        ("page".to_owned(), FalkorValue::I64(i64::from(item.page))),
        (
            "printed_page".to_owned(),
            text_or_null(item.printed_page.as_deref()),
        ),
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

fn count_from_rows(rows: &[Vec<FalkorValue>]) -> Result<u64, String> {
    match rows {
        [row] => match row.as_slice() {
            [FalkorValue::I64(count)] => u64::try_from(*count).map_err(|_| count.to_string()),
            _ => Err(format!("{row:?}")),
        },
        _ => Err(format!("{rows:?}")),
    }
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
