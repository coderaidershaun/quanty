//! Carries the tools to a client: over the standard input and output of the process, or over
//! streamable HTTP on a socket that the caller has bound.

use std::sync::Arc;

use rmcp::ServiceExt;
use rmcp::service::ServerInitializeError;
use rmcp::transport::stdio;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::{StreamableHttpServerConfig, StreamableHttpService};
use tokio::task::JoinError;

use crate::server::QuantyServer;
use crate::services::Services;

const HTTP_PATH: &str = "/mcp";

#[derive(thiserror::Error, Debug)]
pub enum ServeError {
    #[error(
        "could not start the MCP server on the standard input and output; start this command from an MCP client"
    )]
    Start(#[source] Box<ServerInitializeError>),

    #[error("the MCP server stopped without an answer")]
    Stopped(#[source] JoinError),
}

/// Serves the tools on the standard input and output until the client closes them. Nothing but
/// the protocol may be written to the standard output while this runs.
///
/// # Errors
/// - [`ServeError::Start`] when the client does not start the connection
/// - [`ServeError::Stopped`] when the server ends with a panic
pub async fn serve_stdio<S: Services>(server: QuantyServer<S>) -> Result<(), ServeError> {
    let running = server
        .serve(stdio())
        .await
        .map_err(|error| ServeError::Start(Box::new(error)))?;
    running.waiting().await.map_err(ServeError::Stopped)?;
    Ok(())
}

/// Exposes `/mcp` for a server that authenticates every request before dispatch.
/// Browser origins are refused; authenticated CLI clients can use a public host.
pub fn http_service<S: Services>(server: QuantyServer<S>) -> axum::Router {
    let largest_request = usize::try_from(server.largest_request_bytes()).unwrap_or(usize::MAX);
    let config = StreamableHttpServerConfig::default()
        .with_max_request_body_bytes(largest_request)
        .disable_allowed_hosts()
        .enforce_origin_validation();
    let service = StreamableHttpService::new(
        move || Ok(server.clone()),
        Arc::new(LocalSessionManager::default()),
        config,
    );
    axum::Router::new().nest_service(HTTP_PATH, service)
}
