//! What can stop a read or a list of resources, and how each cause reaches the client: a thing
//! that is not there is a missing resource, and anything else is a failure of the server.

use rmcp::ErrorData;
use serde_json::json;

use crate::server::failure::ToolError;

#[derive(thiserror::Error, Debug)]
pub(super) enum ResourceError {
    /// Boxed, because a tool error is large and every call of a reader returns this in its `Err`.
    #[error(transparent)]
    Tool(Box<ToolError>),

    #[error("could not read the arguments of the reader of the resource")]
    Arguments(#[source] serde_json::Error),

    #[error("could not write the view of the resource as JSON")]
    Json(#[source] serde_json::Error),
}

impl ResourceError {
    /// The error of a reader, which words itself the way a tool error does.
    pub(super) fn from_reader(error: impl Into<ToolError>) -> ResourceError {
        ResourceError::Tool(Box::new(error.into()))
    }

    /// The error as the client gets it for the resource at `uri`.
    pub(super) fn for_uri(self, uri: &str) -> ErrorData {
        match self {
            ResourceError::Tool(error) if is_not_found(&error) => {
                ErrorData::resource_not_found(error.text(), Some(json!({ "uri": uri })))
            }
            other => other.internal(),
        }
    }

    /// The error as the client gets it when no single resource is at fault.
    pub(super) fn internal(self) -> ErrorData {
        match self {
            ResourceError::Tool(error) => ErrorData::internal_error(error.text(), None),
            other => ErrorData::internal_error(other.to_string(), None),
        }
    }
}

/// Whether the error says that the thing asked for is not stored, as opposed to a store that is
/// down or a file that cannot be read.
fn is_not_found(error: &ToolError) -> bool {
    match error {
        ToolError::Library(error) => error.is_not_found(),
        ToolError::Insights(error) => error.is_not_found(),
        ToolError::Concepts(error) => error.is_not_found(),
        ToolError::Retrieval(_)
        | ToolError::Ingest(_)
        | ToolError::Export(_)
        | ToolError::Dataset(_)
        | ToolError::Stop(_)
        | ToolError::Watch(_)
        | ToolError::Catalog(_) => false,
    }
}

/// The error for an address that names no resource.
pub(super) fn no_such_resource(uri: &str) -> ErrorData {
    ErrorData::resource_not_found(
        format!(
            "{uri} names no resource of the library; list the resource templates to see the addresses that can be read"
        ),
        Some(json!({ "uri": uri })),
    )
}
