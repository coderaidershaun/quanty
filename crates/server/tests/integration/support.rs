//! Every test owns its listener and an isolated server home.

use backend::fake::Fake;
use contract::{Command, Frame};
use futures_util::{SinkExt, StreamExt};
use mcp::Services;
use ocr::{ChapterJob, ConversionSummary, ConvertError, PageProgress};
use rag_core::{
    AgentCli, Config, EmbedError, GeminiEmbedder, ModelSettings, SettingsError, WorkKind,
};
use rag_ingestion::KeyStates;
use server::{Settings, router};
use std::{path::PathBuf, time::Duration};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream,
    tungstenite::{Message, client::IntoClientRequest},
};

pub type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

pub struct NoPaidServices;

impl Services for NoPaidServices {
    type Embedder = GeminiEmbedder;
    type Llm = AgentCli;

    fn embedder(&self, _: &Config) -> Result<Self::Embedder, EmbedError> {
        panic!("test must not create an embedder")
    }

    fn llm(&self, _: WorkKind) -> Self::Llm {
        panic!("test must not create a model client")
    }

    async fn convert(
        &self,
        _: &ChapterJob,
        _: &Config,
        _: &mut (dyn FnMut(PageProgress) + Send),
    ) -> Result<ConversionSummary, ConvertError> {
        panic!("test must not convert a PDF")
    }

    async fn check_keys(&self, _: &Config) -> KeyStates {
        panic!("test must not check provider keys")
    }

    fn model_choice(&self, _: WorkKind) -> Result<ModelSettings, SettingsError> {
        panic!("test must not load a model choice")
    }
}

pub struct TestServer {
    pub home: tempfile::TempDir,
    pub address: std::net::SocketAddr,
    settings: Settings,
    content: PathBuf,
    background_jobs: mcp::BackgroundJobs,
    mcp_config: Config,
    task: tokio::task::JoinHandle<()>,
}

impl TestServer {
    pub fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.address)
    }
    pub fn client(&self) -> reqwest::Client {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap()
    }
    pub async fn login(&self, client: &reqwest::Client) -> String {
        let response = client
            .post(self.url("/login"))
            .header("content-type", "application/x-www-form-urlencoded")
            .body("password=secret")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 303);
        response.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_owned()
    }
    pub async fn socket(&self) -> Socket {
        let client = self.client();
        let cookie = self.login(&client).await;
        let mut request = format!("ws://{}/api/ws", self.address)
            .into_client_request()
            .unwrap();
        request
            .headers_mut()
            .insert("cookie", cookie.parse().unwrap());
        tokio_tungstenite::connect_async(request).await.unwrap().0
    }
    pub async fn restart(&mut self) {
        self.task.abort();
        self.settings.session_key = Settings::from_sources(self.home.path(), |_| None)
            .unwrap()
            .session_key;
        let (address, task) = start(
            &self.settings,
            &self.content,
            self.background_jobs.clone(),
            self.mcp_config.clone(),
        )
        .await;
        self.address = address;
        self.task = task;
    }
}
impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub async fn test_server(password: Option<&str>) -> TestServer {
    configured(password, |_, _| {}).await
}

pub async fn configured(
    password: Option<&str>,
    configure: impl FnOnce(&mut Settings, &mut PathBuf),
) -> TestServer {
    configured_with_mcp(password, configure, mcp::BackgroundJobs::global(), None).await
}

pub fn closed_mcp_config(home: &std::path::Path) -> Config {
    Config {
        qdrant_url: "http://127.0.0.1:1".to_owned(),
        qdrant_api_key: None,
        falkordb_url: "falkor://127.0.0.1:1".to_owned(),
        falkordb_password: None,
        falkordb_graph: format!("guard-{}", uuid::Uuid::new_v4()),
        items_collection: "guard-items".to_owned(),
        concepts_collection: "guard-concepts".to_owned(),
        concept_cache_folder: home.join("concept-cache"),
        concept_decision_log: home.join("concept-decisions.jsonl"),
        content_folder: home.join("content"),
        dataset_folder: home.join("datasets"),
        dataset_cache_folder: home.join("dataset-cache"),
        gemini_api_key: None,
        jev_api_key: None,
        gemini_ipv4_only: false,
        fetch_origins: Vec::new(),
    }
}

pub async fn configured_with_mcp(
    password: Option<&str>,
    configure: impl FnOnce(&mut Settings, &mut PathBuf),
    background_jobs: mcp::BackgroundJobs,
    mcp_config: Option<Config>,
) -> TestServer {
    let home = tempfile::tempdir().unwrap();
    let mcp_config = mcp_config.unwrap_or_else(|| closed_mcp_config(home.path()));
    let mut content = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples/content");
    let mut settings = Settings::from_sources(home.path(), |_| None).unwrap();
    settings.password = password.map(str::to_owned);
    settings.mcp_token = Some("token".into());
    configure(&mut settings, &mut content);
    let (address, task) = start(
        &settings,
        &content,
        background_jobs.clone(),
        mcp_config.clone(),
    )
    .await;
    TestServer {
        home,
        address,
        settings,
        content,
        background_jobs,
        mcp_config,
        task,
    }
}

async fn start(
    settings: &Settings,
    content: &std::path::Path,
    background_jobs: mcp::BackgroundJobs,
    mcp_config: Config,
) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let mut settings = settings.clone();
    settings.listen = address;
    let fake = Fake::scene("black-scholes", &settings.home).unwrap();
    let mcp = if settings.trusted_integration {
        mcp::fixture_http_service()
    } else {
        mcp::http_service(mcp::QuantyServer::with_background_jobs(
            mcp_config,
            NoPaidServices,
            background_jobs.clone(),
        ))
    };
    let app = router(
        fake,
        server::ServerOptions {
            settings: settings.clone(),
            content: content.to_path_buf(),
            fixture: Some("black-scholes".into()),
            mcp,
            background_jobs,
        },
    )
    .unwrap();
    let task = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
    });
    (address, task)
}

pub async fn send(socket: &mut Socket, command: &Command) {
    socket
        .send(Message::Text(
            serde_json::to_string(command).unwrap().into(),
        ))
        .await
        .unwrap();
}
pub async fn frame(socket: &mut Socket) -> Frame {
    let message = tokio::time::timeout(Duration::from_secs(15), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    serde_json::from_str(message.to_text().unwrap()).unwrap()
}
pub async fn finish(socket: &mut Socket, request: contract::RequestId) -> Vec<contract::Event> {
    let mut events = Vec::new();
    loop {
        match frame(socket).await {
            Frame::Event(event) => events.push(event),
            Frame::Finished(found) => {
                assert_eq!(found, request);
                return events;
            }
        }
    }
}
