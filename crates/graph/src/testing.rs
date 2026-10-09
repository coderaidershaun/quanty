//! Support for tests of other crates: a graph that is removed when the test ends, and reads of
//! what a graph holds. The product cannot read or remove a whole graph, so it is behind a feature.

use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

use falkordb::{FalkorClientBuilder, FalkorConnectionInfo, FalkorValue};
use rag_core::{Config, DocId};

use crate::falkor::FalkorGraph;

const THROWAWAY_PREFIX: &str = "test-graph-";

/// The name of a graph that no one else uses. The graph is deleted when this is dropped, so it
/// goes away also when the test fails. It can only name and remove a graph that starts with the
/// test prefix, so it never touches the real graph. Nothing is created in FalkorDB until the
/// first query.
pub struct ThrowawayGraph {
    name: String,
    url: String,
}

impl ThrowawayGraph {
    pub fn new(config: &Config, test_name: &str) -> Self {
        let nanoseconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("the clock should be after 1970")
            .as_nanos();
        Self {
            name: format!(
                "{THROWAWAY_PREFIX}{test_name}-{}-{nanoseconds}",
                std::process::id()
            ),
            url: config.falkordb_url.clone(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Drop for ThrowawayGraph {
    fn drop(&mut self) {
        if !self.name.starts_with(THROWAWAY_PREFIX) {
            return;
        }
        let name = self.name.clone();
        let url = self.url.clone();
        // On its own thread, so that a panic inside the client ends that thread and not this
        // drop: a panic in a drop while a failed test unwinds stops the whole test run. The
        // blocking client needs no runtime.
        let removed = std::thread::spawn(move || -> Result<(), falkordb::FalkorDBError> {
            let client = FalkorClientBuilder::new()
                .with_connection_info(FalkorConnectionInfo::try_from(url.as_str())?)
                .build()?;
            // A graph that no query reached is not in the list, and asking the store to delete it
            // would be an error.
            if client.list_graphs()?.contains(&name) {
                client.select_graph(&name).delete()?;
            }
            Ok(())
        })
        .join();
        match removed {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                eprintln!(
                    "could not remove the throwaway graph {}: {error}",
                    self.name
                );
            }
            Err(_) => eprintln!(
                "could not remove the throwaway graph {}: the thread that removes it panicked",
                self.name
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredDocument {
    pub title: String,
    /// In reading order.
    pub items: Vec<StoredItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredItem {
    pub id: String,
    pub kind: String,
    pub page: i64,
    pub printed_page: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphSize {
    pub nodes: u64,
    pub edges: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredConcept {
    pub id: String,
    pub name: String,
    pub normalised_name: String,
    pub definition: String,
    pub aliases: Vec<String>,
}

/// A `MENTIONS` edge, as stored. `concept` is the normalised name of the concept, so a test needs
/// no random id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredMention {
    pub item: String,
    pub concept: String,
    pub wording: String,
}

/// A `RELATES_TO` edge, as stored. `from` and `to` are the normalised names of the concepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredRelation {
    pub from: String,
    pub kind: String,
    pub to: String,
    pub item: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredConceptGraph {
    pub concepts: Vec<StoredConcept>,
    pub mentions: Vec<StoredMention>,
    pub relations: Vec<StoredRelation>,
}

/// The document with this id and its items in reading order, or `None` when the graph has no
/// such document.
pub async fn stored_document(graph: &FalkorGraph, id: DocId) -> Option<StoredDocument> {
    let id = id.to_string();
    let titles = read(
        graph,
        "MATCH (d:Document {id: $id}) RETURN d.title AS title",
        &id,
    )
    .await;
    let title_row = titles.into_iter().next()?;
    let title = text(title_row.into_iter().next().expect("a row has a title"));
    let rows = read(
        graph,
        "MATCH (d:Document {id: $id})-[:HAS_ITEM]->(first:Item)
         WHERE indegree(first, 'NEXT') = 0
         MATCH path = (first)-[:NEXT*0..]->(i:Item)<-[:HAS_ITEM]-(d)
         RETURN i.id AS id, i.kind AS kind, i.page AS page, i.printed_page AS printed_page
         ORDER BY length(path)",
        &id,
    )
    .await;
    let items = rows
        .into_iter()
        .map(|row| {
            let mut values = row.into_iter();
            let mut next = || values.next().expect("a row has four values");
            StoredItem {
                id: text(next()),
                kind: text(next()),
                page: number(next()),
                printed_page: optional_text(next()),
            }
        })
        .collect();
    Some(StoredDocument { title, items })
}

pub async fn size(graph: &FalkorGraph) -> GraphSize {
    let count = |rows: Vec<Vec<FalkorValue>>| {
        let value = rows
            .into_iter()
            .next()
            .and_then(|row| row.into_iter().next())
            .expect("a count has one row");
        u64::try_from(number(value)).expect("a count is not negative")
    };
    GraphSize {
        nodes: count(read_all(graph, "MATCH (n) RETURN count(n) AS nodes").await),
        edges: count(read_all(graph, "MATCH ()-[e]->() RETURN count(e) AS edges").await),
    }
}

/// Each label and property that has an index.
pub async fn indexes(graph: &FalkorGraph) -> BTreeSet<(String, String)> {
    let rows = read_all(
        graph,
        "CALL db.indexes() YIELD label, properties RETURN label, properties",
    )
    .await;
    rows.into_iter()
        .flat_map(|row| {
            let mut values = row.into_iter();
            let label = text(values.next().expect("a row has a label"));
            let properties = texts(values.next().expect("a row has a list of properties"));
            properties
                .into_iter()
                .map(move |property| (label.clone(), property))
        })
        .collect()
}

/// Concepts are sorted by normalised name, mentions by item and then concept, and relations by
/// the concept they start at, their kind and the concept they end at.
pub async fn stored_concept_graph(graph: &FalkorGraph) -> StoredConceptGraph {
    StoredConceptGraph {
        concepts: stored_concepts(graph).await,
        mentions: stored_mentions(graph).await,
        relations: stored_relations(graph).await,
    }
}

async fn stored_concepts(graph: &FalkorGraph) -> Vec<StoredConcept> {
    let rows = read_all(
        graph,
        "MATCH (c:Concept)
         RETURN c.id, c.name, c.normalised_name, c.definition, c.aliases
         ORDER BY c.normalised_name",
    )
    .await;
    rows.into_iter()
        .map(|row| {
            let mut values = row.into_iter();
            let mut next = || values.next().expect("a row has five values");
            StoredConcept {
                id: text(next()),
                name: text(next()),
                normalised_name: text(next()),
                definition: text(next()),
                aliases: texts(next()),
            }
        })
        .collect()
}

async fn stored_mentions(graph: &FalkorGraph) -> Vec<StoredMention> {
    let rows = read_all(
        graph,
        "MATCH (i:Item)-[m:MENTIONS]->(c:Concept)
         RETURN i.id, c.normalised_name, m.wording
         ORDER BY i.id, c.normalised_name",
    )
    .await;
    rows.into_iter()
        .map(|row| {
            let mut values = row.into_iter();
            let mut next = || values.next().expect("a row has three values");
            StoredMention {
                item: text(next()),
                concept: text(next()),
                wording: text(next()),
            }
        })
        .collect()
}

async fn stored_relations(graph: &FalkorGraph) -> Vec<StoredRelation> {
    let rows = read_all(
        graph,
        "MATCH (a:Concept)-[r:RELATES_TO]->(b:Concept)
         RETURN a.normalised_name, r.type, b.normalised_name, r.item
         ORDER BY a.normalised_name, r.type, b.normalised_name",
    )
    .await;
    rows.into_iter()
        .map(|row| {
            let mut values = row.into_iter();
            let mut next = || values.next().expect("a row has four values");
            StoredRelation {
                from: text(next()),
                kind: text(next()),
                to: text(next()),
                item: text(next()),
            }
        })
        .collect()
}

async fn read(graph: &FalkorGraph, statement: &str, id: &str) -> Vec<Vec<FalkorValue>> {
    let parameters = vec![("id", FalkorValue::String(id.to_owned()))];
    rows_of(graph, statement, parameters).await
}

async fn read_all(graph: &FalkorGraph, statement: &str) -> Vec<Vec<FalkorValue>> {
    rows_of(graph, statement, Vec::new()).await
}

async fn rows_of(
    graph: &FalkorGraph,
    statement: &str,
    parameters: Vec<(&'static str, FalkorValue)>,
) -> Vec<Vec<FalkorValue>> {
    graph
        .run("read the graph", statement, parameters)
        .await
        .expect("the graph should answer the read")
        .data
        .into_values_lossy()
        .collect()
}

fn text(value: FalkorValue) -> String {
    match value {
        FalkorValue::String(text) => text,
        other => panic!("expected text in the graph, found {other:?}"),
    }
}

fn texts(value: FalkorValue) -> Vec<String> {
    match value {
        FalkorValue::Array(values) => values.into_iter().map(text).collect(),
        other => panic!("expected a list of text in the graph, found {other:?}"),
    }
}

fn optional_text(value: FalkorValue) -> Option<String> {
    match value {
        FalkorValue::None => None,
        other => Some(text(other)),
    }
}

fn number(value: FalkorValue) -> i64 {
    match value {
        FalkorValue::I64(number) => number,
        other => panic!("expected a number in the graph, found {other:?}"),
    }
}
