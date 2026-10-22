//! Checks `stop_job` on an ingest, which is held at the conversion until the test lets it go.

use mcp::QuantyServer;
use ocr::testing::sample_pdf;
use rag_ingestion::testing::ThrowawayStores;
use serde_json::json;

use crate::support::{StandInServices, call, connect, report_when_ended, structured};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p mcp --test integration -- --ignored jobs::"]
async fn stop_job_stops_a_running_ingest_and_a_new_one_can_start() {
    let (services, open_the_gate) = StandInServices::new().with_gate();
    let stores = ThrowawayStores::new("mcp-job-stop");
    let client = connect(QuantyServer::new(stores.config().clone(), services)).await;
    let sent = json!({ "media": "Option Volatility and Pricing", "path": sample_pdf() });
    let first = call(&client, "ingest_pdf", sent.clone()).await;
    let started = structured(&first);
    assert_eq!(started["stage"], "converting");
    let job_id = started["job_id"].as_str().unwrap();

    let stopped = call(&client, "stop_job", json!({ "job_id": job_id })).await;

    assert_eq!(
        structured(&stopped),
        &json!({
            "job_id": job_id,
            "kind": "ingest",
            "state": "stopped",
            "status_tool": "ingest_status",
        })
    );
    let status = call(&client, "ingest_status", json!({ "job_id": job_id })).await;
    let report = structured(&status);
    assert_eq!(report["state"], "stopped", "{report}");
    assert!(report.get("stage").is_none(), "{report}");
    let error = report["error"].as_str().unwrap();
    assert!(error.starts_with("stopped by `stop_job`"), "{error}");
    assert!(
        error.ends_with(
            "Send the same PDF again to go on: converted pages and kept answers are not paid for twice."
        ),
        "{error}"
    );
    let stopped_again = call(&client, "stop_job", json!({ "job_id": job_id })).await;
    assert_eq!(
        structured(&stopped_again)["state"],
        "ended",
        "a job that has ended is left as it is"
    );

    let second = call(&client, "ingest_pdf", sent).await;

    let restarted = structured(&second);
    assert_eq!(restarted["state"], "running", "{restarted}");
    assert_ne!(restarted["job_id"], started["job_id"], "a job of its own");
    open_the_gate.send(()).unwrap();
    let ended = report_when_ended(
        &client,
        "ingest_status",
        restarted["job_id"].as_str().unwrap(),
    )
    .await;
    assert_eq!(ended["state"], "done", "{ended}");
}
