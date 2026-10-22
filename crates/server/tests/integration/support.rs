//! Every test owns its listener and an isolated server home.

use backend::fake::Fake;
use contract::{Command, Frame};
use futures_util::{SinkExt, StreamExt};
use server::{Settings, router};
use std::{path::PathBuf, time::Duration};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream,
    tungstenite::{Message, client::IntoClientRequest},
};

pub type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

pub struct TestServer {
    pub home: tempfile::TempDir,
    pub address: std::net::SocketAddr,
    settings: Settings,
    content: PathBuf,
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
        let (address, task) = start(&self.settings, &self.content).await;
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
    let home = tempfile::tempdir().unwrap();
    let mut content = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples/content");
    let mut settings = Settings::from_sources(home.path(), |_| None).unwrap();
    settings.password = password.map(str::to_owned);
    settings.mcp_token = Some("token".into());
    configure(&mut settings, &mut content);
    let (address, task) = start(&settings, &content).await;
    TestServer {
        home,
        address,
        settings,
        content,
        task,
    }
}

async fn start(
    settings: &Settings,
    content: &std::path::Path,
) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let mut settings = settings.clone();
    settings.listen = address;
    let fake = Fake::scene("black-scholes", &settings.home).unwrap();
    let mcp = mcp::http_service(mcp::QuantyServer::new(
        rag_core::Config::from_sources(|_| None, None).unwrap(),
        mcp::PaidServices::default(),
    ));
    let app = router(
        fake,
        server::ServerOptions {
            settings: settings.clone(),
            content: content.to_path_buf(),
            fixture: Some("black-scholes".into()),
            mcp,
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
