//! A free server that proves an app can reach its host without any store or model client. Its
//! fake ingest lets a host draw the progress of a job with nothing paid for.

mod catalog;
mod fake;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::tool::IntoCallToolResult;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResponse, CallToolResult, ContentBlock, JsonObject};
use rmcp::service::RequestContext;
use rmcp::transport::StreamableHttpServerConfig;
use rmcp::{ErrorData, Json, RoleServer, ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use self::catalog::Catalog;
use self::fake::{FakeIngest, FakeJobs, Progress};
use crate::jobs::{WatchError, WatchJobArgs};
use crate::server::{Cost, Group, ToolMarks, output_schema};
use crate::transport::mount;

const TITLE: &str = "Black–Scholes fixture";
const TEXT: &str = "A free sample for checking the connection between an app and its host.";
const PICTURE: &str = "content/option-volatility-and-pricing/chapter-1/page-num-1/page.png";

#[derive(Clone)]
struct Fixture {
    tool_router: ToolRouter<Self>,
    jobs: FakeJobs,
}

impl Fixture {
    fn new(jobs: FakeJobs) -> Fixture {
        Fixture {
            tool_router: Fixture::tool_router(),
            jobs,
        }
    }
}

/// Why a call of the fixture failed. It reaches the caller as a tool error.
#[derive(thiserror::Error, Debug)]
enum FixtureError {
    #[error("no group of the fixture is named {group:?}; the groups are library, ingest and jobs")]
    UnknownGroup { group: String },

    #[error("no fake job has the id {job_id:?}; call `ingest_pdf` and give the `job_id` it gave")]
    UnknownJob { job_id: String },

    #[error(transparent)]
    Watch(#[from] WatchError),
}

impl IntoCallToolResult for FixtureError {
    fn into_call_tool_result(self) -> Result<CallToolResponse, ErrorData> {
        Ok(CallToolResult::error(vec![ContentBlock::text(self.to_string())]).into())
    }
}

#[derive(Deserialize, JsonSchema)]
struct CatalogArgs {
    /// `library`, `ingest` or `jobs`. Leave it out to see the groups.
    group: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
struct FakeIngestArgs {
    /// Any title. Nothing is looked up.
    media: String,
    /// Any file name. Nothing is read.
    file_name: String,
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

    /// Browse the tools of the fixture by group, free. Leave `group` out to see the groups.
    #[tool(
        title = "Catalog",
        annotations(read_only_hint = true, idempotent_hint = true),
        output_schema = output_schema::<Catalog>(),
        meta = ToolMarks::new(Group::Catalog, Cost::Free)
            .with_discovery(JsonObject::new())
            .into()
    )]
    async fn catalog(
        &self,
        Parameters(args): Parameters<CatalogArgs>,
    ) -> Result<Json<Catalog>, FixtureError> {
        Ok(Json(Catalog::of(args.group.as_deref(), &self.jobs.all())?))
    }

    /// Start a fake ingest, free: nothing is read, written or paid for. The job walks the stages
    /// of a real ingest on a timer. Call `watch_job` with its `job_id` to follow it.
    #[tool(
        title = "Ingest PDF",
        annotations(read_only_hint = false, destructive_hint = false),
        output_schema = output_schema::<FakeIngest>(),
        meta = ToolMarks::new(Group::Ingest, Cost::Free)
            .with_job("watch_job", "job_id")
            .into()
    )]
    async fn ingest_pdf(&self, Parameters(args): Parameters<FakeIngestArgs>) -> Json<FakeIngest> {
        Json(self.jobs.start(args.media, args.file_name).view())
    }

    /// Wait up to `timeout_s` seconds (default 60, at most 90) for a fake job to end, then give
    /// its state. With a progress token it sends a progress notification for each stage.
    #[tool(
        title = "Watch job",
        annotations(read_only_hint = true, idempotent_hint = true),
        output_schema = output_schema::<FakeIngest>(),
        meta = ToolMarks::new(Group::Jobs, Cost::Free)
            .with_job("watch_job", "job_id")
            .into()
    )]
    async fn watch_job(
        &self,
        Parameters(args): Parameters<WatchJobArgs>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<FakeIngest>, FixtureError> {
        let timeout = args.timeout()?;
        let job = self
            .jobs
            .find(&args.job_id)
            .ok_or(FixtureError::UnknownJob {
                job_id: args.job_id,
            })?;
        let progress = context.meta.get_progress_token().map(|token| Progress {
            peer: context.peer.clone(),
            token,
        });
        Ok(Json(job.watch(timeout, progress).await))
    }
}

#[tool_handler(router = self.tool_router, name = "quanty-fixture", instructions = "This fixture has free sample tools and never contacts stores or models: catalog lists them by group, search and fixture_page read the built-in sample, ingest_pdf starts a fake ingest that reads nothing, and watch_job follows it through the stages of a real ingest.")]
impl ServerHandler for Fixture {}

/// A standalone fixture endpoint with no paid operations or external clients.
pub fn fixture_http_service() -> axum::Router {
    // The fake jobs are made here and not in the factory, which runs for each session: a job
    // started in one session is found by `watch_job` in another.
    let jobs = FakeJobs::default();
    mount(
        move || Ok(Fixture::new(jobs.clone())),
        StreamableHttpServerConfig::default(),
    )
}
