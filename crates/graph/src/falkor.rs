//! The connection to FalkorDB, the check that the store behind it answers, and the statements
//! that write and remove documents and items.

use std::collections::HashMap;
use std::time::Duration;

use falkordb::{
    AsyncGraph, FalkorAsyncClient, FalkorClientBuilder, FalkorConnectionInfo, FalkorDBError,
    FalkorValue, QueryResult, RowStream,
};
use rag_core::{Config, DocId};

use crate::store::{DocumentNode, GraphError, GraphStore, ItemNode};

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
