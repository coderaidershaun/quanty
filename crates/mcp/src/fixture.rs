//! A free catalogue proves app connections without constructing store or model clients.

use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{Json, ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Clone)]
struct Fixture {
    tool_router: ToolRouter<Self>,
}

#[derive(Serialize, JsonSchema)]
struct Page {
    title: String,
    text: String,
    /// Resolve this contained picture URL against the app's base address.
    image: String,
}

#[tool_router]
impl Fixture {
    /// Read one built-in sample page. This local fixture needs no account or paid service.
    #[tool(annotations(read_only_hint = true, destructive_hint = false))]
    async fn fixture_page(&self) -> Json<Page> {
        Json(Page {
            title: "Black–Scholes fixture".into(),
            text: "A free sample for checking the connection between an app and its host.".into(),
            image: "content/option-volatility-and-pricing/chapter-1/page-num-1/page.png".into(),
        })
    }
}

#[tool_handler(router = self.tool_router, name = "quanty-fixture", instructions = "This fixture has one free sample tool and never contacts stores or models.")]
impl ServerHandler for Fixture {}

/// A standalone fixture endpoint with no paid operations or external clients.
pub fn fixture_http_service() -> axum::Router {
    let service = StreamableHttpService::new(
        || {
            Ok(Fixture {
                tool_router: Fixture::tool_router(),
            })
        },
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default()
            .disable_allowed_hosts()
            .enforce_origin_validation(),
    );
    axum::Router::new().nest_service("/mcp", service)
}
