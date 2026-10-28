//! The connection to FalkorDB, the check that the store behind it answers, and the graph it
//! prepares for the statements.

mod aliases;
mod concepts;
mod documents;
mod edges;
mod indexes;
mod mentions;
mod records;
mod traversal;
mod writes;

use std::pin::Pin;
use std::str::FromStr;
use std::time::Duration;

use falkordb::{
    AsyncGraph, FalkorAsyncClient, FalkorClientBuilder, FalkorConnectionInfo, FalkorDBError,
    FalkorValue, QueryResult, RowStream,
};
use rag_core::{ApiKey, Config};

use crate::error::GraphError;

const RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);

/// An open connection to FalkorDB, and the graph that the config names.
///
/// Every write can be repeated: a second call with the same values changes nothing.
pub struct FalkorGraph {
    client: FalkorAsyncClient,
    graph: AsyncGraph,
    url: String,
}

impl FalkorGraph {
    /// Makes the graph and its indexes when they are missing, so a store that is down fails here
    /// and a graph that nothing was written to yet reads as an empty library. The client needs the
    /// multi-thread runtime of tokio.
    ///
    /// # Errors
    /// - [`GraphError::Connect`] when the address is not valid or nothing answers there
    /// - [`GraphError::Ping`] and [`GraphError::NotFalkorDb`] as for [`FalkorGraph::ping`]
    /// - [`GraphError::Query`] when the indexes cannot be read or made
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a list
    ///   of indexes
    pub async fn connect(config: &Config) -> Result<FalkorGraph, GraphError> {
        let url = config.falkordb_url.clone();
        let connect_error = |source| GraphError::Connect {
            url: config.falkordb_url.clone(),
            source,
        };
        let info = FalkorConnectionInfo::try_from(url.as_str()).map_err(connect_error)?;
        let info = match &config.falkordb_password {
            Some(password) => with_password(info, password),
            None => info,
        };
        let building = FalkorClientBuilder::new_async()
            .with_connection_info(info)
            .with_response_timeout(Some(RESPONSE_TIMEOUT))
            .build();
        // Every future of the FalkorDB client is boxed where it is made. They are so deeply
        // nested that, left inside the future of a caller, they make the compiler stop with
        // "queries overflow the depth limit".
        let client = Box::pin(building).await.map_err(connect_error)?;
        let graph = client.select_graph(&config.falkordb_graph);
        let connected = FalkorGraph { client, graph, url };
        // Asked first, so that another program on the port is reported as not FalkorDB and not as
        // a statement that failed.
        connected.ping().await?;
        indexes::ensure(&connected).await?;
        Ok(connected)
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
        // The boxed future also gets a type that hides what is inside it. Otherwise, to prove that
        // a caller's future can move to another thread, the compiler walks every future nested in
        // the client, and it stops with an overflow error on a caller that spawns a whole ingest.
        let query: Pin<Box<dyn Future<Output = _> + Send + '_>> = Box::pin(query);
        query.await.map_err(|source| GraphError::Query {
            url: self.url.clone(),
            graph: self.graph.graph_name().to_owned(),
            action,
            source,
        })
    }
}

/// The connection with the password set on it and not written into the address, so that no error
/// that names the address can print it.
fn with_password(info: FalkorConnectionInfo, password: &ApiKey) -> FalkorConnectionInfo {
    // Only the `tokio` feature of falkordb is on, so `Redis` is the one variant. A build that
    // turns on the embedded server fails to compile here, so the password is never dropped.
    let FalkorConnectionInfo::Redis(redis) = info;
    let settings = redis
        .redis_settings()
        .clone()
        .set_password(password.expose());
    FalkorConnectionInfo::Redis(redis.set_redis_settings(settings))
}

/// An id as the text that Qdrant also uses for it.
fn id_value(id: impl ToString) -> FalkorValue {
    FalkorValue::String(id.to_string())
}

fn unreadable_reply(
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

/// The reads of many rows share this, so that each reports a row it cannot read in the same way.
fn read_rows<T, C: FromIterator<T>>(
    graph: &FalkorGraph,
    action: &'static str,
    expected: &'static str,
    reply: QueryResult<RowStream>,
    from_row: impl FnMut(Vec<FalkorValue>) -> Result<T, String>,
) -> Result<C, GraphError> {
    reply
        .data
        .into_values_lossy()
        .map(from_row)
        .collect::<Result<_, _>>()
        .map_err(|found| unreadable_reply(graph, action, expected, found))
}

/// A row of exactly `N` values.
fn fixed<const N: usize>(row: Vec<FalkorValue>) -> Result<[FalkorValue; N], String> {
    <[FalkorValue; N]>::try_from(row).map_err(|row| format!("{row:?}"))
}

fn parsed<T: FromStr>(text: &str) -> Result<T, String> {
    text.parse().map_err(|_| format!("{text:?}"))
}

/// The texts of an array, each one made into a `T`. Null is an empty array.
fn texts_into<T, C: FromIterator<T>>(
    value: FalkorValue,
    make: impl Fn(String) -> Result<T, String>,
) -> Result<C, String> {
    let items = match value {
        FalkorValue::None => Vec::new(),
        FalkorValue::Array(items) => items,
        other => return Err(format!("{other:?}")),
    };
    items
        .into_iter()
        .map(|item| match item {
            FalkorValue::String(text) => make(text),
            other => Err(format!("{other:?}")),
        })
        .collect()
}

fn id_list(ids: &[impl ToString + Copy]) -> FalkorValue {
    FalkorValue::Array(ids.iter().copied().map(id_value).collect())
}
