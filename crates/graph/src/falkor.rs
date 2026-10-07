//! The connection to FalkorDB, the check that the store behind it answers, and the statements
//! that write documents, items and concepts, find a concept, and remove a document.

use std::collections::HashMap;
use std::time::Duration;

use falkordb::{
    AsyncGraph, FalkorAsyncClient, FalkorClientBuilder, FalkorConnectionInfo, FalkorDBError,
    FalkorValue, QueryResult, RowStream,
};
use rag_core::{ConceptId, Config, DocId};

use crate::store::{
    ConceptAlias, ConceptNode, DocumentNode, GraphError, GraphStore, ItemNode, Mention, Relation,
};

const RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);
/// Rows go in groups of this size, so that one statement stays quick also when the graph is
/// large: the client waits only a short time for a reply.
const ROWS_PER_STATEMENT: usize = 200;

const UPSERT_DOCUMENT: &str = "MERGE (d:Document {id: $id}) SET d.title = $title";

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

/// An open connection to FalkorDB, and the graph that the config names.
pub struct FalkorGraph {
    client: FalkorAsyncClient,
    graph: AsyncGraph,
    url: String,
}

impl FalkorGraph {
    /// Opens the connection, so a store that is down fails here. The client needs the
    /// multi-thread runtime of tokio.
    ///
    /// # Errors
    /// [`GraphError::Connect`] when the address is not valid or nothing answers there.
    pub async fn connect(config: &Config) -> Result<FalkorGraph, GraphError> {
        let url = config.falkordb_url.clone();
        let connect_error = |source| GraphError::Connect {
            url: config.falkordb_url.clone(),
            source,
        };
        let info = FalkorConnectionInfo::try_from(url.as_str()).map_err(connect_error)?;
        let building = FalkorClientBuilder::new_async()
            .with_connection_info(info)
            .with_response_timeout(Some(RESPONSE_TIMEOUT))
            .build();
        // Every future of the FalkorDB client is boxed where it is made. They are so deeply
        // nested that, left inside the future of a caller, they make the compiler stop with
        // "queries overflow the depth limit".
        let client = Box::pin(building).await.map_err(connect_error)?;
        let graph = client.select_graph(&config.falkordb_graph);
        Ok(FalkorGraph { client, graph, url })
    }

    /// Asks the store for its list of graphs, which is the cheapest request that needs the
    /// store to be working.
    ///
    /// # Errors
    /// - [`GraphError::Ping`] when the store does not answer
    /// - [`GraphError::NotFalkorDb`] when something answers that is not FalkorDB
    pub async fn ping(&self) -> Result<(), GraphError> {
        let url = self.url.clone();
        // Boxed for the same reason as the future in `connect`.
        Box::pin(self.client.list_graphs())
            .await
            .map(|_| ())
            .map_err(|source| match source {
                FalkorDBError::ParsingArray => GraphError::NotFalkorDb { url, source },
                source => GraphError::Ping { url, source },
            })
    }

    /// The only place that waits for a query. It boxes the future for the same reason as in
    /// `connect`.
    pub(crate) async fn run(
        &self,
        action: &'static str,
        statement: &str,
        parameters: Vec<(&'static str, FalkorValue)>,
    ) -> Result<QueryResult<RowStream>, GraphError> {
        // Each query needs the graph handle by `&mut`, and a clone is cheap.
        let mut graph = self.graph.clone();
        let query = graph.query(statement).with_params(parameters).execute();
        Box::pin(query).await.map_err(|source| GraphError::Query {
            url: self.url.clone(),
            graph: self.graph.graph_name().to_owned(),
            action,
            source,
        })
    }
}

impl GraphStore for FalkorGraph {
    async fn upsert_document(&self, document: &DocumentNode) -> Result<(), GraphError> {
        let parameters = vec![
            ("id", id_value(document.id)),
            ("title", FalkorValue::String(document.title.clone())),
        ];
        self.run("write the document", UPSERT_DOCUMENT, parameters)
            .await?;
        Ok(())
    }

    async fn upsert_items(&self, document: DocId, items: &[ItemNode]) -> Result<(), GraphError> {
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

    async fn delete_document(&self, id: DocId) -> Result<u64, GraphError> {
        let reply = self
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

    async fn upsert_concept(&self, concept: &ConceptNode) -> Result<(), GraphError> {
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
        self.run("write the concept", UPSERT_CONCEPT, parameters)
            .await?;
        Ok(())
    }

    async fn add_mentions(&self, mentions: &[Mention]) -> Result<(), GraphError> {
        for group in mentions.chunks(ROWS_PER_STATEMENT) {
            let rows = group.iter().map(mention_row).collect();
            let parameters = vec![("mentions", FalkorValue::Array(rows))];
            self.run("write the mentions", ADD_MENTIONS, parameters)
                .await?;
        }
        Ok(())
    }

    async fn add_relations(&self, relations: &[Relation]) -> Result<(), GraphError> {
        for group in relations.chunks(ROWS_PER_STATEMENT) {
            let rows = group.iter().map(relation_row).collect();
            let parameters = vec![("relations", FalkorValue::Array(rows))];
            self.run("write the relations", ADD_RELATIONS, parameters)
                .await?;
        }
        Ok(())
    }

    async fn add_alias(&self, alias: &ConceptAlias) -> Result<(), GraphError> {
        let parameters = vec![
            ("id", id_value(alias.concept)),
            ("name", FalkorValue::String(alias.name.clone())),
            (
                "normalised_name",
                FalkorValue::String(alias.normalised_name.clone()),
            ),
        ];
        self.run("add an alias to the concept", ADD_ALIAS, parameters)
            .await?;
        Ok(())
    }

    async fn find_concept_by_name(
        &self,
        normalised_name: &str,
    ) -> Result<Option<ConceptNode>, GraphError> {
        let parameters = vec![("name", FalkorValue::String(normalised_name.to_owned()))];
        self.read_concept("look up a concept by its name", FIND_CONCEPT, parameters)
            .await
    }

    async fn concept(&self, id: ConceptId) -> Result<Option<ConceptNode>, GraphError> {
        let parameters = vec![("id", id_value(id))];
        self.read_concept("read a concept by its id", CONCEPT_BY_ID, parameters)
            .await
    }
}

impl FalkorGraph {
    /// The first row of the reply as a concept, or `None` when the reply has no row.
    async fn read_concept(
        &self,
        action: &'static str,
        statement: &str,
        parameters: Vec<(&'static str, FalkorValue)>,
    ) -> Result<Option<ConceptNode>, GraphError> {
        let reply = self.run(action, statement, parameters).await?;
        let Some(row) = reply.data.into_values_lossy().next() else {
            return Ok(None);
        };
        concept_from_row(row)
            .map(Some)
            .map_err(|found| GraphError::UnreadableReply {
                url: self.url.clone(),
                graph: self.graph.graph_name().to_owned(),
                action,
                found,
            })
    }
}

/// Reads a row of four texts: an id, a name, a normalised name and a definition. Any other row
/// comes back as the text that the error shows.
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

/// An id as the text that Qdrant also uses for it.
fn id_value(id: impl ToString) -> FalkorValue {
    FalkorValue::String(id.to_string())
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
