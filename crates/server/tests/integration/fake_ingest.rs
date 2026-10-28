//! The fake ingest of the trusted fixture: its catalogue, and a job that one session starts and
//! another follows through every stage with a progress notification for each.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use rmcp::model::ProgressNotificationParam;
use rmcp::service::{NotificationContext, RunningService};
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::{ClientHandler, RoleClient, ServiceExt};
use serde_json::json;

use super::mcp::{error_text, tool_call};
use super::support::TestServer;

/// A client that keeps every progress notification the fixture sends it.
#[derive(Clone, Default)]
struct StageNotices(Arc<Mutex<Vec<ProgressNotificationParam>>>);

impl ClientHandler for StageNotices {
    async fn on_progress(
        &self,
        params: ProgressNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        self.0.lock().unwrap().push(params);
    }
}

/// What the fixture says for each stage of the fake ingest, in order.
const FAKE_INGEST_STAGES: [&str; 11] = [
    "checking what is already stored",
    "opening the PDF",
    "preparing the pages: 7 of 7 ready",
    "converting the pages: 7 of 7 done",
    "writing artifact descriptions: 20 of 20",
    "writing the graph",
    "embedding 20 items",
    "storing the items",
    "writing the insights, takeaways and cheat sheet",
    "reading the concepts: 20 of 20",
    "linking the concepts: 20 of 20",
];

/// The catalogue shows the groups and then the tools of one, and an ingest that one session
/// starts is followed by another session through every stage, with a progress notification for
/// each, to its end.
pub(super) async fn a_fake_ingest_is_followed_through_its_stages(
    server: &TestServer,
    starter: &RunningService<RoleClient, ()>,
) {
    let groups = starter
        .call_tool(tool_call("catalog", json!({})))
        .await
        .unwrap();
    let groups = groups.structured_content.unwrap();
    assert_eq!(groups["version"], 1);
    let leads: Vec<_> = groups["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| {
            (
                node["kind"].clone(),
                node["tool"].clone(),
                node["arguments"].clone(),
            )
        })
        .collect();
    assert_eq!(
        leads,
        ["library", "ingest", "jobs"].map(|group| (
            json!("discover"),
            json!("catalog"),
            json!({ "group": group })
        ))
    );
    let ingest_group = starter
        .call_tool(tool_call("catalog", json!({ "group": "ingest" })))
        .await
        .unwrap();
    assert_eq!(
        ingest_group.structured_content.unwrap()["nodes"][0]["tool"],
        "ingest_pdf"
    );

    let started = starter
        .call_tool(tool_call(
            "ingest_pdf",
            json!({ "media": "Black–Scholes fixture", "file_name": "chapter-1-sample.pdf" }),
        ))
        .await
        .unwrap();
    let started = started.structured_content.unwrap();
    assert_eq!(started["state"], "running", "{started}");
    assert_eq!(started["kind"], "ingest");
    assert_eq!(started["file_name"], "chapter-1-sample.pdf");
    let job_id = started["job_id"].as_str().unwrap().to_owned();

    let stages = StageNotices::default();
    let transport = StreamableHttpClientTransport::with_client(
        server.client(),
        StreamableHttpClientTransportConfig::with_uri(server.url("/mcp")),
    );
    let watcher = stages.clone().serve(transport).await.unwrap();
    let ended = watcher
        .call_tool(tool_call(
            "watch_job",
            json!({ "job_id": job_id, "timeout_s": 30 }),
        ))
        .await
        .unwrap();
    let ended = ended.structured_content.unwrap();
    assert_eq!(ended["state"], "done", "{ended}");
    assert_eq!(ended["job_id"], job_id.as_str());
    assert!(ended.get("stage").is_none(), "{ended}");

    // A notification can reach the client after the answer of the call that caused it.
    let waited = std::time::Instant::now();
    while stages
        .0
        .lock()
        .unwrap()
        .last()
        .and_then(|notice| notice.message.as_deref())
        != FAKE_INGEST_STAGES.last().copied()
    {
        assert!(
            waited.elapsed() < Duration::from_secs(10),
            "the last stage never came: {:?}",
            stages.0.lock().unwrap()
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let seen = stages.0.lock().unwrap().clone();
    let messages: Vec<&str> = seen
        .iter()
        .map(|notice| notice.message.as_deref().unwrap_or_default())
        .collect();
    // The watcher may have begun after the first stage, but it sees the rest in order.
    assert!(FAKE_INGEST_STAGES.ends_with(&messages), "{messages:?}");
    assert!(messages.len() >= 5, "{messages:?}");
    // The bar is for the whole job: it rises with every notification and ends full.
    let values: Vec<f64> = seen.iter().map(|notice| notice.progress).collect();
    assert!(
        values.windows(2).all(|pair| pair[0] < pair[1]),
        "{values:?}"
    );
    assert_eq!(values.last(), Some(&100.0), "{values:?}");
    assert!(
        seen.iter().all(|notice| notice.total == Some(100.0)),
        "{seen:?}"
    );

    let jobs = starter
        .call_tool(tool_call("catalog", json!({ "group": "jobs" })))
        .await
        .unwrap();
    assert_eq!(
        jobs.structured_content.unwrap()["nodes"][0]["arguments"],
        json!({ "job_id": job_id })
    );
    let unknown = watcher
        .call_tool(tool_call(
            "watch_job",
            json!({ "job_id": "00000000-0000-4000-8000-000000000000" }),
        ))
        .await
        .unwrap();
    assert!(error_text(&unknown).contains("no fake job"));
    watcher.cancel().await.unwrap();
}
