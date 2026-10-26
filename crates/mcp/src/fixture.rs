//! A free catalogue proves app connections without constructing store or model clients.

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::transport::StreamableHttpServerConfig;
use rmcp::{Json, ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::transport::mount;

const TITLE: &str = "Black–Scholes fixture";
const TEXT: &str = "A free sample for checking the connection between an app and its host.";
const PICTURE: &str = "content/option-volatility-and-pricing/chapter-1/page-num-1/page.png";

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

#[derive(Deserialize, JsonSchema)]
struct SearchQuestion {
    /// Any question returns the same free sample figure.
    question: String,
}

#[derive(Serialize, JsonSchema)]
struct SearchResult {
    results: Vec<Figure>,
}

#[derive(Serialize, JsonSchema)]
struct Figure {
    number: u32,
    document: String,
    document_id: String,
    page: u32,
    kind: String,
    text: String,
    /// Resolve this contained picture URL against the app's base address.
    picture: String,
    label: String,
}

#[tool_router]
impl Fixture {
    /// Search the built-in sample, free. Every question returns one numbered figure; no stores or models are contacted.
    #[tool(annotations(read_only_hint = true, destructive_hint = false))]
    async fn search(&self, Parameters(question): Parameters<SearchQuestion>) -> Json<SearchResult> {
        let _ = question.question;
        Json(SearchResult {
            results: vec![Figure {
                number: 1,
                document: TITLE.into(),
                document_id: "black-scholes-fixture".into(),
                page: 1,
                kind: "figure".into(),
                text: TEXT.into(),
                picture: PICTURE.into(),
                label: "Black–Scholes sample page".into(),
            }],
        })
    }

    /// Read one built-in sample page. This local fixture needs no account or paid service.
    #[tool(annotations(read_only_hint = true, destructive_hint = false))]
    async fn fixture_page(&self) -> Json<Page> {
        Json(Page {
            title: TITLE.into(),
            text: TEXT.into(),
            image: PICTURE.into(),
        })
    }
}

#[tool_handler(router = self.tool_router, name = "quanty-fixture", instructions = "This fixture has two free sample tools, fixture_page and search, and never contacts stores or models.")]
impl ServerHandler for Fixture {}

/// A standalone fixture endpoint with no paid operations or external clients.
pub fn fixture_http_service() -> axum::Router {
    mount(
        || {
            Ok(Fixture {
                tool_router: Fixture::tool_router(),
            })
        },
        StreamableHttpServerConfig::default(),
    )
}
