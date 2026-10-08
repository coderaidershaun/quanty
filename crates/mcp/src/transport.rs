//! Carries the tools to a client: over the standard input and output of the process, or over
//! streamable HTTP on a socket that the caller has bound.

use std::sync::Arc;

use rmcp::ServiceExt;
use rmcp::service::ServerInitializeError;
use rmcp::transport::stdio;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::{StreamableHttpServerConfig, StreamableHttpService};
use tokio::net::TcpListener;
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

    #[error("the HTTP server stopped")]
    Http(#[source] std::io::Error),

    #[error(
        "a request can be {bytes} bytes, which is more than this machine can hold; give the server a smaller limit on the size of a PDF"
    )]
    RequestTooBig { bytes: u64 },
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

/// Serves the tools over streamable HTTP at `/mcp` on the socket of `listener`, until the
/// listener fails. Only a `Host` of this machine is accepted, which stops a web page from reaching
/// the server through another name. The caller binds the listener, and binds it to a loopback
/// address only: the server has no login.
///
/// # Errors
/// - [`ServeError::RequestTooBig`] when the request size that a PDF needs does not fit in memory
/// - [`ServeError::Http`] when the listener fails
pub async fn serve_http<S: Services>(
    server: QuantyServer<S>,
    listener: TcpListener,
) -> Result<(), ServeError> {
    let largest_request = server.largest_request_bytes();
    let largest_request =
        usize::try_from(largest_request).map_err(|_| ServeError::RequestTooBig {
            bytes: largest_request,
        })?;
    // The default of 4 MiB is too small for a chapter PDF sent as base64.
    let config = StreamableHttpServerConfig::default().with_max_request_body_bytes(largest_request);
    let service = StreamableHttpService::new(
        move || Ok(server.clone()),
        Arc::new(LocalSessionManager::default()),
        config,
    );
    let router = axum::Router::new().nest_service(HTTP_PATH, service);
    axum::serve(listener, router)
        .await
        .map_err(ServeError::Http)
}
