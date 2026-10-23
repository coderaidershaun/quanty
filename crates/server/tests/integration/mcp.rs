//! Agent requests authenticate on every method, independently of browser sessions.

use super::support::{closed_mcp_config, configured, configured_with_mcp, test_server};
use contract::{Command, DatasetRequest, Event, Frame, JobKind, RequestId, StartupFacts};
use rmcp::{
    ServiceExt,
    model::CallToolRequestParams,
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde_json::json;
use std::time::Duration;

fn graph_config(home: &std::path::Path, address: std::net::SocketAddr) -> rag_core::Config {
    assert_eq!(
        address.ip(),
        std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
    );
    assert_ne!(address.port(), 0);
    assert_ne!(address.port(), 6379);
    assert_ne!(address.port(), 6334);
    let mut config = closed_mcp_config(home);
    config.falkordb_url = format!("falkor://{address}");
    assert_eq!(config.qdrant_url, "http://127.0.0.1:1");
    assert!(config.falkordb_url.starts_with("falkor://127.0.0.1:"));
    assert!(config.falkordb_graph.starts_with("guard-"));
    assert!(config.qdrant_api_key.is_none());
    assert!(config.falkordb_password.is_none());
    assert!(config.gemini_api_key.is_none());
    assert!(config.jev_api_key.is_none());
    for path in [
        &config.concept_cache_folder,
        &config.concept_decision_log,
        &config.content_folder,
        &config.dataset_folder,
        &config.dataset_cache_folder,
    ] {
        assert!(
            path.starts_with(home),
            "test paths must stay in the temporary home"
        );
    }
    config
}

fn tool_call(name: &'static str, args: serde_json::Value) -> CallToolRequestParams {
    CallToolRequestParams::new(name).with_arguments(args.as_object().unwrap().clone())
}

fn error_text(result: &rmcp::model::CallToolResult) -> String {
    assert_eq!(result.is_error, Some(true));
    result
        .content
        .iter()
        .filter_map(|part| part.as_text())
        .map(|text| text.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

async fn facts(server: &super::support::TestServer) -> StartupFacts {
    let client = server.client();
    let cookie = server.login(&client).await;
    client
        .get(server.url("/api/facts"))
        .header("cookie", cookie)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

#[tokio::test]
async fn browser_and_mcp_share_one_background_job_and_truthful_facts() {
    let closed_graph = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let graph_address = closed_graph.local_addr().unwrap();
    let store_home = tempfile::tempdir().unwrap();
    let server = configured_with_mcp(
        Some("secret"),
        |_, _| {},
        mcp::BackgroundJobs::new(),
        Some(graph_config(store_home.path(), graph_address)),
    )
    .await;
    let mut browser = server.socket().await;
    super::support::send(
        &mut browser,
        &Command::BuildDataset {
            request: RequestId(901),
            build: DatasetRequest {
                name: "browser-job".into(),
                ..Default::default()
            },
        },
    )
    .await;
    assert!(matches!(
        super::support::frame(&mut browser).await,
        Frame::Event(Event::DatasetProgress { .. })
    ));
    let mut config = StreamableHttpClientTransportConfig::with_uri(server.url("/mcp"));
    config.auth_header = Some("token".into());
    let transport = StreamableHttpClientTransport::with_client(server.client(), config);
    let agent = ().serve(transport).await.unwrap();
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::select! {
            result = agent.call_tool(tool_call("build_dataset", json!({"name":"agent-job"}))) => result,
            accepted = closed_graph.accept() => {
                let (socket, _) = accepted.unwrap();
                drop(socket);
                panic!("MCP attempted graph preparation while the browser job held admission");
            }
        }
    })
    .await
    .unwrap()
    .unwrap();
    let text = error_text(&result);
    assert!(text.contains("background work is running"), "{text}");
    assert!(text.contains("browser-job"), "{text}");
    assert_eq!(facts(&server).await.running[0].kind, JobKind::Dataset);
    assert!(matches!(
        &super::support::finish(&mut browser, RequestId(901)).await[..],
        [.., Event::DatasetFinished { result: Ok(_), .. }]
    ));
    agent.cancel().await.unwrap();
    drop(closed_graph);

    for (tool, args, kind, name) in [
        (
            "ingest_pdf",
            json!({"media":"Sample", "path": format!("{}/../../samples/chapter-1-sample-pages.pdf", env!("CARGO_MANIFEST_DIR"))}),
            JobKind::Ingest,
            "chapter-1-sample-pages.pdf",
        ),
        (
            "build_dataset",
            json!({"name":"agent-dataset"}),
            JobKind::Dataset,
            "agent-dataset",
        ),
        (
            "write_insights",
            json!({"all":true}),
            JobKind::Insights,
            "all documents",
        ),
    ] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let graph_address = listener.local_addr().unwrap();
        let store_home = tempfile::tempdir().unwrap();
        let server = configured_with_mcp(
            Some("secret"),
            |_, _| {},
            mcp::BackgroundJobs::new(),
            Some(graph_config(store_home.path(), graph_address)),
        )
        .await;
        let mut config = StreamableHttpClientTransportConfig::with_uri(server.url("/mcp"));
        config.auth_header = Some("token".into());
        let transport = StreamableHttpClientTransport::with_client(server.client(), config);
        let agent = std::sync::Arc::new(().serve(transport).await.unwrap());
        let calling = agent.clone();
        let pending = tokio::spawn(async move { calling.call_tool(tool_call(tool, args)).await });
        let (socket, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
            .await
            .unwrap()
            .unwrap();
        let shown = facts(&server).await;
        assert_eq!(shown.running.len(), 1);
        assert_eq!(shown.running[0].kind, kind);
        assert_eq!(shown.running[0].name, name);
        let status = tokio::time::timeout(
            Duration::from_secs(5),
            agent.call_tool(tool_call(
                "dataset_status",
                json!({"job_id":"00000000-0000-0000-0000-000000000000"}),
            )),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(error_text(&status).contains("no dataset build has"));
        let mut browser = server.socket().await;
        super::support::send(
            &mut browser,
            &Command::LoadCatalogue {
                request: RequestId(10),
            },
        )
        .await;
        assert!(matches!(
            &super::support::finish(&mut browser, RequestId(10)).await[..],
            [Event::Catalogue { result: Ok(_), .. }]
        ));
        super::support::send(
            &mut browser,
            &Command::BuildDataset {
                request: RequestId(11),
                build: DatasetRequest {
                    name: "browser-later".into(),
                    ..Default::default()
                },
            },
        )
        .await;
        assert!(
            matches!(&super::support::finish(&mut browser, RequestId(11)).await[..], [Event::DatasetFinished { result: Err(failure), .. }] if failure.detail == "an ingest is running" && failure.hint.contains(name))
        );
        assert_eq!(facts(&server).await.running[0].kind, kind);
        assert!(!pending.is_finished());
        drop(socket);
        let closing = tokio::spawn(async move {
            loop {
                let (socket, _) = listener.accept().await.unwrap();
                drop(socket);
            }
        });
        let result = tokio::time::timeout(Duration::from_secs(15), pending)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(result.is_error, Some(true));
        closing.abort();
        closing.await.unwrap_err();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if facts(&server).await.running.is_empty() {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        super::support::send(
            &mut browser,
            &Command::BuildDataset {
                request: RequestId(12),
                build: DatasetRequest {
                    name: "browser-after".into(),
                    ..Default::default()
                },
            },
        )
        .await;
        assert!(matches!(
            &super::support::finish(&mut browser, RequestId(12)).await[..],
            [.., Event::DatasetFinished { result: Ok(_), .. }]
        ));
        drop(agent);
    }
}

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
