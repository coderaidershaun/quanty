//! Agent requests authenticate on every method, independently of browser sessions.

use super::support::{configured, test_server};
use rmcp::{
    ServiceExt,
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};

#[tokio::test]
async fn mcp_requires_bearer_and_accepts_public_hosts_but_refuses_browser_origins() {
    let server = test_server(Some("secret")).await;
    let client = server.client();
    for method in [
        reqwest::Method::GET,
        reqwest::Method::POST,
        reqwest::Method::DELETE,
    ] {
        for token in [None, Some("wrong")] {
            let mut request = client
                .request(method.clone(), server.url("/mcp"))
                .header("host", "quanty.example.com");
            if let Some(token) = token {
                request = request.bearer_auth(token);
            }
            assert_eq!(request.send().await.unwrap().status(), 401);
        }
    }
    let response = client.post(server.url("/mcp")).bearer_auth("token").header("host", "quanty.example.com").header("origin", "https://quanty.example.com").header("content-type", "application/json").header("accept", "application/json, text/event-stream").body(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#).send().await.unwrap();
    assert_eq!(response.status(), 403);
    let mut config = StreamableHttpClientTransportConfig::with_uri(server.url("/mcp"));
    config.auth_header = Some("token".into());
    config.custom_headers.insert(
        "host".parse().unwrap(),
        "quanty.example.com".parse().unwrap(),
    );
    let transport = StreamableHttpClientTransport::with_client(client.clone(), config);
    let running = ().serve(transport).await.unwrap();
    assert!(!running.list_all_tools().await.unwrap().is_empty());
    running.cancel().await.unwrap();
    let disabled = configured(None, |settings, _| settings.mcp_token = None).await;
    assert_eq!(
        client
            .post(disabled.url("/mcp"))
            .bearer_auth("token")
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
}
