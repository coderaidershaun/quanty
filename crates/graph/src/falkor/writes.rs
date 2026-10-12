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
use crate::error::GraphError;

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

impl FalkorGraph {
    /// Creates the document node, or updates its title and its labels: a label that `document`
    /// does not have is taken away from the stored node. Repeating it changes nothing.
    ///
    /// # Errors
    /// [`GraphError::Query`] when the store refuses or cannot be reached.
    pub async fn upsert_document(&self, document: &DocumentNode) -> Result<(), GraphError> {
        let labels = &document.labels;
        let parameters = vec![
            ("id", id_value(document.id)),
            ("title", text_value(&document.title)),
            ("media", text_or_null(labels.media.as_deref())),
            (
                "category",
                text_or_null(labels.category.map(|category| category.as_str())),
            ),
            ("authors", text_list(&labels.authors)),
            ("media_tags", tag_list(&labels.media_tags)),
            ("tags", tag_list(&labels.tags)),
        ];
        self.run("write the document", UPSERT_DOCUMENT, parameters)
            .await?;
        Ok(())
    }

    /// Creates the media node. A media with exactly this title that is in the graph stays as it
    /// is, with the labels it has, so an ingest never changes a stored media and repeating it
    /// changes nothing. Titles are compared as they are given: the caller decides whether two
    /// spellings name one media.
    ///
    /// # Errors
    /// [`GraphError::Query`] when the store refuses or cannot be reached.
    pub async fn add_media(&self, media: &MediaNode) -> Result<(), GraphError> {
        write_media(self, "write the media", ADD_MEDIA, media).await
    }

    /// Creates the media node, or writes its category, its authors and its tags over the ones it
    /// has. Titles are compared as they are given.
    ///
    /// # Errors
    /// [`GraphError::Query`] when the store refuses or cannot be reached.
    pub async fn update_media(&self, media: &MediaNode) -> Result<(), GraphError> {
        write_media(self, "write the labels of the media", UPDATE_MEDIA, media).await
    }

    /// Removes the media node with exactly this title, and says whether there was one. Titles are
    /// compared as they are given: the caller decides whether two spellings name one media. The
    /// documents that carry the title are not touched: a media has no edge.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a count
    pub async fn delete_media(&self, title: &str) -> Result<bool, GraphError> {
        let parameters = vec![("title", text_value(title))];
        let removed = run_counted(self, "delete the media", DELETE_MEDIA, parameters).await?;
        Ok(removed > 0)
    }

    /// Records whether the document is ingested whole. `Some(items)` marks it, with the number of
    /// its items. `None` takes the mark away. A document that is not in the graph is not created.
    /// Repeating it changes nothing.
    ///
    /// # Errors
    /// [`GraphError::Query`] when the store refuses or cannot be reached.
    pub async fn set_ingested_items(
        &self,
        document: DocId,
        items: Option<u64>,
    ) -> Result<(), GraphError> {
        // FalkorDB removes a property that is set to null.
        let items = match items {
            Some(count) => FalkorValue::I64(i64::try_from(count).unwrap_or(i64::MAX)),
            None => FalkorValue::None,
        };
        let parameters = vec![("id", id_value(document)), ("items", items)];
        self.run(
            "record whether the document is ingested whole",
            SET_INGESTED_ITEMS,
            parameters,
        )
        .await?;
        Ok(())
    }

    /// Records the folder the document was ingested from. A document that is not in the graph is
    /// not created. Repeating it changes nothing.
    ///
    /// # Errors
    /// - [`GraphError::FolderNotUnicode`] when the path of the folder is not valid Unicode, which
    ///   the graph cannot keep
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    pub async fn set_chapter_folder(
        &self,
        document: DocId,
        folder: &Path,
    ) -> Result<(), GraphError> {
        let text = folder
            .to_str()
            .ok_or_else(|| GraphError::FolderNotUnicode {
                folder: folder.to_path_buf(),
            })?;
        let parameters = vec![("id", id_value(document)), ("folder", text_value(text))];
        self.run(
            "record the folder the document was ingested from",
            SET_CHAPTER_FOLDER,
            parameters,
        )
        .await?;
        Ok(())
    }

    /// Writes the items of a document: a node for each item, an edge `HAS_ITEM` from the
    /// document to each item, and an edge `NEXT` from each item to the one after it. Creates the
    /// document node when it is missing. Repeating it adds no node and no edge. An empty slice
    /// makes no call.
    ///
    /// `items` must be every item of the document, in reading order, in one call. A second call
    /// with the rest of the items does not join the two parts with a `NEXT` edge.
    ///
    /// # Errors
    /// [`GraphError::Query`] when the store refuses or cannot be reached.
    pub async fn upsert_items(
        &self,
        document: DocId,
        items: &[ItemNode],
    ) -> Result<(), GraphError> {
        for group in items.chunks(ROWS_PER_STATEMENT) {
            let rows = group.iter().map(item_row).collect();
            let parameters = vec![
                ("document", id_value(document)),
                ("items", FalkorValue::Array(rows)),
            ];
            self.run("write the items", UPSERT_ITEMS, parameters)
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
            self.run("write the order of the items", LINK_NEXT, parameters)
                .await?;
        }
        Ok(())
    }

    /// Removes the document node, its item nodes and every edge of those nodes, the `MENTIONS`
    /// edges of the items among them. Concepts and their `RELATES_TO` edges stay: they belong to
    /// no document. Returns how many nodes it removed. Zero means the graph has no such document,
    /// which is not an error.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a count
    pub async fn delete_document(&self, id: DocId) -> Result<u64, GraphError> {
        let parameters = vec![("id", id_value(id))];
        run_counted(self, "delete the document", DELETE_DOCUMENT, parameters).await
    }

    /// Creates the concept node with no aliases, or writes its name, normalised name and
    /// definition again. It leaves the aliases of a stored concept as they are. Repeating it
    /// changes nothing.
    ///
    /// # Errors
    /// [`GraphError::Query`] when the store refuses or cannot be reached.
    pub async fn upsert_concept(&self, concept: &ConceptNode) -> Result<(), GraphError> {
        let parameters = vec![
            ("id", id_value(concept.id)),
            ("name", text_value(&concept.name)),
            ("normalised_name", text_value(&concept.normalised_name)),
            ("definition", text_value(&concept.definition)),
        ];
        self.run("write the concept", UPSERT_CONCEPT, parameters)
            .await?;
        Ok(())
    }

    /// Writes one `MENTIONS` edge from an item to a concept for each mention, with the wording.
    /// An item mentions a concept once: a second mention of the same pair replaces the wording.
    /// An empty slice makes no call.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::MissingNodes`] when the item or the concept of a mention is not in the
    ///   graph. That mention is not written, and every other one is.
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a count
    pub async fn add_mentions(&self, mentions: &[Mention]) -> Result<(), GraphError> {
        let mut written = 0;
        for group in mentions.chunks(ROWS_PER_STATEMENT) {
            let rows = group.iter().map(mention_row).collect();
            let parameters = vec![("mentions", FalkorValue::Array(rows))];
            written += run_counted(self, "write the mentions", ADD_MENTIONS, parameters).await?;
        }
        all_written(self, "mentions", mentions.len(), written)
    }

    /// Writes one `RELATES_TO` edge for each relation. Two concepts have at most one edge of a
    /// kind: it keeps the item that stated it first. An empty slice makes no call.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::MissingNodes`] when a concept of a relation is not in the graph. That
    ///   relation is not written, and every other one is.
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a count
    pub async fn add_relations(&self, relations: &[Relation]) -> Result<(), GraphError> {
        let mut written = 0;
        for group in relations.chunks(ROWS_PER_STATEMENT) {
            let rows = group.iter().map(relation_row).collect();
            let parameters = vec![("relations", FalkorValue::Array(rows))];
            written += run_counted(self, "write the relations", ADD_RELATIONS, parameters).await?;
        }
        all_written(self, "relations", relations.len(), written)
    }

    /// Adds the name to the concept. A name is not added when the concept already has its
    /// normalised form, as its name or as an alias, so repeating it changes nothing. A concept
    /// that is not in the graph is not created.
    ///
    /// # Errors
    /// [`GraphError::Query`] when the store refuses or cannot be reached.
    pub async fn add_alias(&self, alias: &ConceptAlias) -> Result<(), GraphError> {
        let parameters = vec![
            ("id", id_value(alias.concept)),
            ("name", text_value(&alias.name)),
            ("normalised_name", text_value(&alias.normalised_name)),
        ];
        self.run("add an alias to the concept", ADD_ALIAS, parameters)
            .await?;
        Ok(())
    }
}

/// Adding a media and changing its labels send the same four parameters.
async fn write_media(
    graph: &FalkorGraph,
    action: &'static str,
    statement: &str,
    media: &MediaNode,
) -> Result<(), GraphError> {
    let parameters = vec![
        ("title", text_value(&media.title)),
        ("category", text_value(media.labels.category.as_str())),
        ("authors", text_list(&media.labels.authors)),
        ("tags", tag_list(&media.labels.tags)),
    ];
    graph.run(action, statement, parameters).await?;
    Ok(())
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

/// Most parameters of a write are text, and each one needs its own copy of that text.
fn text_value(text: &str) -> FalkorValue {
    FalkorValue::String(text.to_owned())
}

fn text_or_null(text: Option<&str>) -> FalkorValue {
    text.map_or(FalkorValue::None, text_value)
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
        ("kind".to_owned(), text_value(item.kind.as_str())),
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
        ("wording".to_owned(), text_value(&mention.wording)),
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
        ("type".to_owned(), text_value(relation.kind.as_str())),
        ("item".to_owned(), id_value(relation.item)),
    ]))
}
