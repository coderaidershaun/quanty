//! An in-process MCP client over a pipe, and the readers of what a tool call gave back.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mcp::{QuantyServer, Services};
use rmcp::model::{
    CallToolRequest, CallToolRequestParams, CallToolResult, ClientCapabilities, ClientConfig,
    ClientRequest, Implementation, ProgressNotificationParam, ProtocolVersion,
};
use rmcp::service::{NotificationContext, PeerRequestOptions, RunningService, Service};
use rmcp::{ClientHandler, ClientLifecycleMode, ClientServiceExt, RoleClient, ServiceExt};
use serde_json::{Map, Value};
use tokio::io::DuplexStream;

pub type Client = RunningService<RoleClient, ()>;

/// A client that keeps every progress notification the server sends it.
#[derive(Clone, Default)]
pub struct ProgressRecorder {
    seen: Arc<Mutex<Vec<ProgressNotificationParam>>>,
}

impl ClientHandler for ProgressRecorder {
    async fn on_progress(
        &self,
        params: ProgressNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        self.seen.lock().unwrap().push(params);
    }
}

impl ProgressRecorder {
    /// The first notification once one has come. The client may handle a notification after it has
    /// handled the response of the call that caused it, so this waits for it a while.
    pub async fn first(&self) -> ProgressNotificationParam {
        let started = Instant::now();
        loop {
            if let Some(first) = self.seen.lock().unwrap().first() {
                return first.clone();
            }
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "no progress notification came"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
}

/// A client over a pipe that records the progress notifications of the server.
pub async fn connect_recording_progress<S: Services>(
    server: QuantyServer<S>,
) -> (
    RunningService<RoleClient, ProgressRecorder>,
    ProgressRecorder,
) {
    let recorder = ProgressRecorder::default();
    let client = recorder
        .clone()
        .serve(serve_on_pipe(server))
        .await
        .expect("the client should connect");
    (client, recorder)
}

/// Sends a call of a tool and cancels it as soon as it is sent, as a client does when the person
/// closes the card of a call. Gives what the client got back for the call.
pub async fn call_and_cancel<S: Service<RoleClient>>(
    client: &RunningService<RoleClient, S>,
    tool: &'static str,
    arguments: Value,
) -> Result<(), rmcp::ServiceError> {
    let Value::Object(arguments) = arguments else {
        panic!("the arguments of a tool call are a JSON object, not {arguments}");
    };
    let request = ClientRequest::CallToolRequest(CallToolRequest::new(
        CallToolRequestParams::new(tool).with_arguments(arguments),
    ));
    let sent = client
        .send_cancellable_request(request, PeerRequestOptions::no_options())
        .await?;
    sent.cancel(Some("the test closed the call".to_owned()))
        .await
}

/// A client that negotiated the 2025-11-25 protocol with an `initialize` handshake.
pub async fn connect<S: Services>(server: QuantyServer<S>) -> Client {
    ().serve(serve_on_pipe(server))
        .await
        .expect("the client should connect")
}

/// A client that negotiated the 2026-07-28 protocol, which has no `initialize` handshake: the
/// client asks the server to discover its versions instead.
pub async fn connect_on_2026_07_28<S: Services>(server: QuantyServer<S>) -> Client {
    ().serve_with_lifecycle(
        serve_on_pipe(server),
        ClientLifecycleMode::Discover {
            preferred_versions: vec![ProtocolVersion::V_2026_07_28],
        },
    )
    .await
    .expect("the client should connect")
}

/// What a client that declares the tasks extension tells the server about itself.
pub fn tasks_client_config() -> ClientConfig {
    ClientConfig::new(
        ClientCapabilities::builder().enable_tasks().build(),
        Implementation::new("quanty-test-client", "0.0.0"),
    )
}

/// A client that negotiated the 2026-07-28 protocol and declared the tasks extension, so that the
/// start tools answer it with a task.
pub async fn connect_declaring_tasks<S: Services>(
    server: QuantyServer<S>,
) -> RunningService<RoleClient, ClientConfig> {
    tasks_client_config()
        .serve_with_lifecycle(
            serve_on_pipe(server),
            ClientLifecycleMode::Discover {
                preferred_versions: vec![ProtocolVersion::V_2026_07_28],
            },
        )
        .await
        .expect("the client should connect")
}

/// Starts `server` on one end of a pipe and gives back the other end.
fn serve_on_pipe<S: Services>(server: QuantyServer<S>) -> DuplexStream {
    let (server_end, client_end) = tokio::io::duplex(1 << 20);
    tokio::spawn(async move {
        let running = server
            .serve(server_end)
            .await
            .expect("the server should start");
        running
            .waiting()
            .await
            .expect("the server should end without a panic");
    });
    client_end
}

/// Calls a tool and gives back the whole result, an error result too.
pub async fn call<S: Service<RoleClient>>(
    client: &RunningService<RoleClient, S>,
    tool: &'static str,
    arguments: Value,
) -> CallToolResult {
    let arguments: Map<String, Value> = match arguments {
        Value::Object(map) => map,
        other => panic!("the arguments of a tool call are a JSON object, not {other}"),
    };
    client
        .call_tool(CallToolRequestParams::new(tool).with_arguments(arguments))
        .await
        .unwrap_or_else(|error| panic!("the call of {tool} should get a result: {error}"))
}

pub fn structured(result: &CallToolResult) -> &Value {
    assert_ne!(
        result.is_error,
        Some(true),
        "the call failed: {}",
        text_of(result)
    );
    result
        .structured_content
        .as_ref()
        .expect("a tool result should be structured")
}

/// The text of a call that failed, which is a tool error and not a protocol error.
pub fn error_text(result: &CallToolResult) -> String {
    assert_eq!(
        result.is_error,
        Some(true),
        "the call should have failed, but gave {:?}",
        result.structured_content
    );
    text_of(result)
}

fn text_of(result: &CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|block| block.as_text())
        .map(|text| text.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}
