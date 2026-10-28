//! Checks the dataset tools over throwaway stores and the committed sample chapters: a second build
//! while one runs, a build that cannot start, and a dry run to the end with its report.

use std::time::Duration;

use mcp::QuantyServer;
use rag_core::Config;
use rag_ingestion::testing::{StandInLlm, ThrowawayStores};
use serde_json::json;
use tokio::time::timeout;

use crate::support::{
    ReadyGate, StandInServices, call, connect, error_text, report_when_ended,
    sample_content_folder, structured,
};

/// How long a test waits for a step that it expects at once, so that a failure does not hang.
const STEP_WAIT: Duration = Duration::from_secs(60);

/// A server over the throwaway stores that reads the committed sample chapters.
fn server_over_samples(
    stores: &ThrowawayStores,
    services: StandInServices,
) -> QuantyServer<StandInServices> {
    let config = Config {
        content_folder: sample_content_folder(),
        ..stores.config().clone()
    };
    QuantyServer::new(config, services)
}

async fn next_sign_in_check(gate: &mut ReadyGate) {
    timeout(STEP_WAIT, gate.reached.recv())
        .await
        .expect("a sign-in check should come to the gate")
        .expect("the gate should stay open for the checks");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p mcp --test integration -- --ignored jobs::datasets::"]
async fn a_second_build_is_refused_while_one_runs_and_names_the_running_job() {
    let stores = ThrowawayStores::new("mcp-dataset-busy");
    let (services, mut gate) = StandInServices::new().with_ready_gate();
    let server = server_over_samples(&stores, services);
    let first_client = connect(server.clone()).await;
    let second_client = connect(server).await;
    // The check of the tool goes on at once. The check of the build waits, with its job listed.
    gate.open.send(()).unwrap();
    // With no texts, questions or examples to write, the build asks the model nothing, whatever
    // its stages plan.
    let first = tokio::spawn(async move {
        call(
            &first_client,
            "build_dataset",
            json!({
                "name": "first",
                "rewrites": 0,
                "questions": 0,
                "max_synthetic": 0,
                "max_sft": 0
            }),
        )
        .await
    });
    next_sign_in_check(&mut gate).await;
    next_sign_in_check(&mut gate).await;

    let second = call(
        &second_client,
        "build_dataset",
        json!({ "name": "second", "dry_run": true }),
    )
    .await;

    let refusal = error_text(&second);
    drop(gate.open);
    let answered = timeout(STEP_WAIT, first)
        .await
        .expect("the first build should answer once it is let go on")
        .expect("the first call should not panic");
    let started = structured(&answered);
    let job_id = started["job_id"].as_str().unwrap();
    assert!(refusal.contains(job_id), "{refusal}");
    assert!(refusal.contains("`job_status`"), "{refusal}");
    let ended = report_when_ended(&second_client, "dataset_status", job_id).await;
    assert_eq!(ended["state"], "done", "{ended}");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p mcp --test integration -- --ignored jobs::datasets::"]
async fn a_build_that_cannot_start_says_what_to_do_and_a_dry_run_reports_its_folder() {
    let stores = ThrowawayStores::new("mcp-dataset-refused");
    let client = connect(server_over_samples(&stores, StandInServices::new())).await;

    let bad_name = call(
        &client,
        "build_dataset",
        json!({ "name": "../x", "dry_run": true }),
    )
    .await;
    let text = error_text(&bad_name);
    assert!(text.contains("\"../x\""), "{text}");
    assert!(text.contains("letters, digits"), "{text}");
    assert!(!text.contains("`health`"), "no service is down: {text}");

    let unknown_document = call(
        &client,
        "build_dataset",
        json!({ "name": "t", "documents": ["No Such Document"], "dry_run": true }),
    )
    .await;
    let text = error_text(&unknown_document);
    assert!(text.contains("No Such Document"), "{text}");
    assert!(text.contains("`list_documents`"), "{text}");
    assert!(!text.contains("`health`"), "no service is down: {text}");

    let signed_out = StandInLlm::signed_out();
    let services = StandInServices::new().with_llm(signed_out.clone());
    let client = connect(server_over_samples(&stores, services)).await;
    let paid = call(&client, "build_dataset", json!({ "name": "t" })).await;
    let text = error_text(&paid);
    assert!(text.contains("not signed in"), "{text}");
    assert!(text.contains("`dry_run: true`"), "{text}");
    assert!(text.contains("Call the `health` tool"), "{text}");
    assert_eq!(signed_out.calls(), 0, "no question is asked");

    let dry = call(
        &client,
        "build_dataset",
        json!({ "name": "t", "dry_run": true }),
    )
    .await;
    let started = structured(&dry);
    assert_eq!(started["kind"], "dataset", "{started}");
    assert!(started["started_at"].is_string(), "{started}");
    let ended = report_when_ended(
        &client,
        "dataset_status",
        started["job_id"].as_str().unwrap(),
    )
    .await;
    assert_eq!(ended["state"], "done", "a dry run needs no model: {ended}");
    assert_eq!(ended["kind"], "dataset", "{ended}");
    assert!(ended["updated_at"].is_string(), "{ended}");
    let folder = stores.config().dataset_folder.join("t");
    assert_eq!(ended["folder"], folder.display().to_string());
    assert!(folder.join("cpt/raw.jsonl").is_file());
    assert!(folder.join("manifest.json").is_file());
    let summary = ended["summary"].as_str().unwrap();
    assert!(summary.contains("dry run"), "{summary}");
    assert!(ended["usage"].is_array(), "{ended}");
    assert!(ended.get("cost_usd").is_some(), "{ended}");
    assert!(
        ended.get("stage").is_none() && ended.get("error").is_none(),
        "{ended}"
    );
}
