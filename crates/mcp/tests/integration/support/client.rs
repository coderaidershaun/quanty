//! An in-process MCP client over a pipe, and the readers of what a tool call gave back.

use mcp::{QuantyServer, Services};
use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::service::RunningService;
use rmcp::{RoleClient, ServiceExt};
use serde_json::{Map, Value};

pub type Client = RunningService<RoleClient, ()>;

pub async fn connect<S: Services>(server: QuantyServer<S>) -> Client {
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
    ().serve(client_end)
        .await
        .expect("the client should connect")
}

/// Calls a tool and gives back the whole result, an error result too.
pub async fn call(client: &Client, tool: &'static str, arguments: Value) -> CallToolResult {
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
