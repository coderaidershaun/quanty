//! The connection to FalkorDB, and the check that the store behind it answers.

use std::time::Duration;

use falkordb::{FalkorAsyncClient, FalkorClientBuilder, FalkorConnectionInfo, FalkorDBError};
use rag_core::Config;

const RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);

/// An open connection to FalkorDB.
pub struct FalkorGraph {
    client: FalkorAsyncClient,
    url: String,
}

#[derive(thiserror::Error, Debug)]
pub enum GraphError {
    #[error("could not connect to FalkorDB at {url}")]
    Connect {
        url: String,
        #[source]
        source: FalkorDBError,
    },

    #[error("FalkorDB at {url} did not answer a request for its list of graphs")]
    Ping {
        url: String,
        #[source]
        source: FalkorDBError,
    },

    // A plain Redis on the same port answers the request with an error reply, which the client
    // can only report as a reply that is not a list.
    #[error(
        "the server at {url} did not answer like FalkorDB; another program may be using that port"
    )]
    NotFalkorDb {
        url: String,
        #[source]
        source: FalkorDBError,
    },
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
        Ok(FalkorGraph { client, url })
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
}
