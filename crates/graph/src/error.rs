//! The ways a request of the graph store can fail.

use std::path::PathBuf;

use falkordb::FalkorDBError;

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

    #[error("FalkorDB at {url} failed to {action} in the graph {graph}")]
    Query {
        url: String,
        graph: String,
        action: &'static str,
        #[source]
        source: FalkorDBError,
    },

    #[error(
        "FalkorDB at {url} answered the request to {action} in the graph {graph} with {found}, which is not {expected}"
    )]
    UnreadableReply {
        url: String,
        graph: String,
        action: &'static str,
        found: String,
        /// What the reply should have been, in words that fit after "which is not".
        expected: &'static str,
    },

    #[error(
        "FalkorDB at {url} wrote {written} of the {asked} {edges} in the graph {graph}; the others name an item or a concept that the graph does not hold"
    )]
    MissingNodes {
        url: String,
        graph: String,
        /// The kind of edge, in the plural: mentions or relations.
        edges: &'static str,
        asked: u64,
        written: u64,
    },

    #[error(
        "the folder {} cannot be kept in the graph because its path is not valid Unicode",
        folder.display()
    )]
    FolderNotUnicode { folder: PathBuf },
}
