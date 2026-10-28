//! Checks the tools that follow a job of any kind: `job_status`, `watch_job` and `list_jobs`. A job
//! is held in a stage by a gate of the stand-in services, so each test sees the job in a state it
//! chose, and then lets it go on.

use std::time::{Duration, Instant};

use contract::IngestStage;
use mcp::{BackgroundJobs, QuantyServer};
use ocr::testing::sample_pdf;
use rag_ingestion::testing::ThrowawayStores;
use serde_json::{Value, json};

use crate::support::{
    Library, STAGE_WAIT, StandInServices, call, call_and_cancel, connect, copy_samples, error_text,
    report_when_ended, sample_content_folder, status_in_stage, structured,
};

const MEDIA: &str = "Option Volatility and Pricing";

/// The words a job view must have, whatever the kind of job.
const STAGE_KEYS: [&str; 5] = ["done", "name", "share", "total", "words"];

/// Stand-in services over empty throwaway stores, whose content folder is where the chapter is
/// converted. A shared background slot is given when a test looks at it.
fn empty_library(
    test_name: &str,
    services: StandInServices,
    slot: Option<BackgroundJobs>,
) -> (ThrowawayStores, QuantyServer<StandInServices>) {
    let stores = ThrowawayStores::new(test_name);
    let config = stores.config().clone();
    let server = match slot {
        Some(slot) => QuantyServer::with_background_jobs(config, services, slot),
        None => QuantyServer::new(config, services),
    };
    (stores, server)
}

fn stage_keys(stage: &Value) -> Vec<&str> {
    let mut keys: Vec<&str> = stage
        .as_object()
        .expect("a stage is an object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    keys
}

/// What every job view has: its kind and state, the two times, and no nesting of the kind fields.
fn assert_job_view(view: &Value, kind: &str) {
    assert_eq!(view["kind"], kind, "{view}");
    assert!(view["job_id"].is_string(), "{view}");
    for time in ["started_at", "updated_at"] {
        let text = view[time]
            .as_str()
            .unwrap_or_else(|| panic!("{time}: {view}"));
        assert!(text.contains('T') && text.ends_with('Z'), "{time}: {text}");
    }
    assert!(view.get("job").is_none(), "no nesting: {view}");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p mcp --test integration -- --ignored jobs::"]
async fn job_status_gives_one_job_view_for_an_ingest_an_insights_job_and_a_dataset_build() {
    an_ingest_is_seen_before_and_after_its_pages_are_counted().await;
    let library = Library::ingested_with_concepts("mcp-jobs-views").await;
    an_insights_job_is_seen_in_the_document_it_asks_about(&library).await;
    a_dataset_build_is_seen_in_its_stage(&library).await;
}

async fn an_ingest_is_seen_before_and_after_its_pages_are_counted() {
    // The first gate holds the conversion before it starts, so the job sits in `opening_pdf`,
    // which does not count. The second holds each page at its first paid call, so the job sits in
    // `converting` with every page cut and none saved.
    let (services, open_the_conversion) = StandInServices::new().with_gate();
    let (services, open_the_pages) = services.with_page_gate();
    let (_stores, server) = empty_library("mcp-jobs-ingest", services, None);
    let client = connect(server).await;

    let started = call(
        &client,
        "ingest_pdf",
        json!({ "media": MEDIA, "path": sample_pdf() }),
    )
    .await;

    let started = structured(&started);
    assert_job_view(started, "ingest");
    assert_eq!(started["state"], "running", "{started}");
    assert_eq!(started["media"], MEDIA);
    let job_id = started["job_id"].as_str().unwrap();
    let opening = status_in_stage(&client, job_id, "opening_pdf").await;
    assert_job_view(&opening, "ingest");
    assert_eq!(
        opening["stage"],
        json!({
            "name": "opening_pdf",
            "words": "opening the PDF",
            "done": null,
            "total": null,
            "share": null,
        })
    );

    open_the_conversion.send(()).unwrap();
    let converting = status_in_stage(&client, job_id, "converting").await;

    let share = IngestStage::Converting { saved: 0, pages: 7 }
        .share_done()
        .unwrap();
    assert_eq!(stage_keys(&converting["stage"]), STAGE_KEYS);
    assert_eq!(
        converting["stage"],
        json!({
            "name": "converting",
            "words": "converting the pages: 0 of 7 done",
            "done": 0,
            "total": 7,
            "share": (f64::from(share) * 1000.0).round() / 1000.0,
        })
    );
    assert_eq!(converting["media"], MEDIA, "the kind fields stay beside");
    assert!(converting["file"].is_string(), "{converting}");

    drop(open_the_pages);
    let ended = report_when_ended(&client, "job_status", job_id).await;
    assert_job_view(&ended, "ingest");
    assert_eq!(ended["state"], "done", "{ended}");
    assert!(ended.get("stage").is_none(), "{ended}");
    assert!(ended["document_id"].is_string(), "{ended}");
    assert!(ended["usage"].is_array(), "{ended}");
}

async fn an_insights_job_is_seen_in_the_document_it_asks_about(library: &Library) {
    let content = tempfile::tempdir().unwrap();
    copy_samples(content.path());
    let (services, mut questions) = StandInServices::new().with_ask_gate();
    let client = connect(library.server_over(content.path(), services)).await;

    let started = call(&client, "write_insights", json!({ "all": true })).await;

    let started = structured(&started);
    assert_job_view(started, "insights");
    assert_eq!(started["state"], "running", "{started}");
    assert_eq!(started["dry_run"], false);
    let title = started["current"].as_str().expect("a question is out");
    let stage = &started["stage"];
    assert_eq!(stage_keys(stage), STAGE_KEYS);
    assert_eq!(stage["name"], "writing_insights");
    assert_eq!((&stage["done"], &stage["total"]), (&json!(0), &json!(3)));
    assert_eq!(stage["share"], 0.0);
    assert!(stage["words"].as_str().unwrap().contains(title), "{stage}");
    let job_id = started["job_id"].as_str().unwrap();
    let held = structured(&call(&client, "job_status", json!({ "job_id": job_id })).await).clone();
    assert_eq!(held["stage"], started["stage"]);
    assert_eq!(held["current"], started["current"], "the kind fields stay");

    questions.reached.recv().await.expect("a question is held");
    drop(questions.open);
    let ended = report_when_ended(&client, "insights_status", job_id).await;
    assert_job_view(&ended, "insights");
    assert_eq!(ended["state"], "done", "{ended}");
    assert!(ended.get("stage").is_none(), "{ended}");
    assert_eq!(ended["documents"].as_array().unwrap().len(), 3, "{ended}");
    assert!(ended["usage"].is_array() && ended.get("cost_usd").is_some());
}

async fn a_dataset_build_is_seen_in_its_stage(library: &Library) {
    let (services, _questions) = StandInServices::new().with_ask_gate();
    let client = connect(library.server_over(&sample_content_folder(), services)).await;

    let started = call(
        &client,
        "build_dataset",
        json!({
            "name": "seen",
            "rewrites": 1,
            "questions": 0,
            "max_sft": 0,
            "max_synthetic": 1,
        }),
    )
    .await;

    let started = structured(&started);
    assert_job_view(started, "dataset");
    assert_eq!(started["state"], "running", "{started}");
    let job_id = started["job_id"].as_str().unwrap();
    let held = status_in_stage(&client, job_id, "writing_synthetic").await;
    assert_job_view(&held, "dataset");
    assert_eq!(stage_keys(&held["stage"]), STAGE_KEYS);
    assert_eq!(held["stage"]["done"], 0, "{held}");
    assert_eq!(held["stage"]["total"], 1, "{held}");
    assert_eq!(held["stage"]["share"], 0.0, "{held}");
    assert!(held["folder"].is_string(), "the kind fields stay: {held}");

    let stopped = call(&client, "stop_job", json!({ "job_id": job_id })).await;
    assert_eq!(structured(&stopped)["state"], "stopped");
    let ended = report_when_ended(&client, "job_status", job_id).await;
    assert_eq!(ended["state"], "stopped", "{ended}");
    assert!(ended.get("stage").is_none(), "{ended}");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p mcp --test integration -- --ignored jobs::"]
async fn watch_job_returns_when_the_stage_changes_or_at_its_timeout() {
    let (services, open_the_pages) = StandInServices::new().with_page_gate();
    let (_stores, server) = empty_library("mcp-jobs-watch", services, None);
    let client = connect(server).await;
    let started = call(
        &client,
        "ingest_pdf",
        json!({ "media": MEDIA, "path": sample_pdf() }),
    )
    .await;
    let job_id = structured(&started)["job_id"].as_str().unwrap().to_owned();
    status_in_stage(&client, &job_id, "converting").await;

    // Nothing changes, so the call ends at its timeout with the job as it is.
    let began = Instant::now();
    let unchanged = call(
        &client,
        "watch_job",
        json!({ "job_id": job_id, "timeout_s": 1 }),
    )
    .await;
    assert!(began.elapsed() >= Duration::from_millis(900));
    assert!(began.elapsed() < Duration::from_secs(10));
    let unchanged = structured(&unchanged);
    assert_eq!(unchanged["state"], "running", "{unchanged}");
    assert_eq!(unchanged["stage"]["name"], "converting", "{unchanged}");
    assert_eq!(unchanged["media"], MEDIA, "the same result as job_status");

    // One page is saved while the call waits. The count moves and the stage does not, so the call
    // still waits for its timeout.
    let began = Instant::now();
    let (counted, ()) = tokio::join!(
        call(
            &client,
            "watch_job",
            json!({ "job_id": job_id, "timeout_s": 3 })
        ),
        async {
            open_the_pages.send(()).unwrap();
            let one_page_saved = async {
                loop {
                    let status = call(&client, "job_status", json!({ "job_id": job_id })).await;
                    if structured(&status)["stage"]["done"] == 1 {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            };
            tokio::time::timeout(STAGE_WAIT, one_page_saved)
                .await
                .expect("a page should be saved");
        }
    );
    assert!(
        began.elapsed() >= Duration::from_millis(2900),
        "{:?}",
        began.elapsed()
    );
    let counted = structured(&counted);
    assert_eq!(counted["stage"]["name"], "converting", "{counted}");

    // The other pages go on, and the stage after the pages ends the wait long before its timeout.
    let began = Instant::now();
    let (changed, ()) = tokio::join!(
        call(
            &client,
            "watch_job",
            json!({ "job_id": job_id, "timeout_s": 90 })
        ),
        async { drop(open_the_pages) }
    );
    assert!(
        began.elapsed() < Duration::from_secs(80),
        "{:?}",
        began.elapsed()
    );
    let changed = structured(&changed);
    let still_converting =
        changed["state"] == "running" && changed["stage"]["name"] == "converting";
    assert!(!still_converting, "{changed}");

    // A job that has ended is returned at once, whatever the timeout.
    report_when_ended(&client, "job_status", &job_id).await;
    let began = Instant::now();
    let ended = call(
        &client,
        "watch_job",
        json!({ "job_id": job_id, "timeout_s": 90 }),
    )
    .await;
    assert!(began.elapsed() < Duration::from_secs(5));
    assert_eq!(structured(&ended)["state"], "done");

    for timeout_s in [0, 91, -5] {
        let refused = call(
            &client,
            "watch_job",
            json!({ "job_id": job_id, "timeout_s": timeout_s }),
        )
        .await;
        let text = error_text(&refused);
        assert!(text.contains("from 1 to 90"), "{text}");
    }
    let unknown = call(&client, "watch_job", json!({ "job_id": "no-such-job" })).await;
    let text = error_text(&unknown);
    assert!(text.contains("no job of this server has the id"), "{text}");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p mcp --test integration -- --ignored jobs::"]
async fn a_cancelled_watch_leaves_its_job_running() {
    let slot = BackgroundJobs::new();
    let (services, open_the_pages) = StandInServices::new().with_page_gate();
    let (_stores, server) = empty_library("mcp-jobs-cancel", services, Some(slot.clone()));
    let client = connect(server).await;
    let started = call(
        &client,
        "ingest_pdf",
        json!({ "media": MEDIA, "path": sample_pdf() }),
    )
    .await;
    let job_id = structured(&started)["job_id"].as_str().unwrap().to_owned();
    let before = status_in_stage(&client, &job_id, "converting").await;

    call_and_cancel(
        &client,
        "watch_job",
        json!({ "job_id": job_id, "timeout_s": 90 }),
    )
    .await
    .expect("the cancel should be sent");

    // The job was not stopped, aborted or changed, and it still holds the shared slot.
    let after = structured(&call(&client, "job_status", json!({ "job_id": job_id })).await).clone();
    assert_eq!(after["state"], "running", "{after}");
    assert_eq!(after["stage"], before["stage"]);
    assert_eq!(after["updated_at"], before["updated_at"]);
    assert_eq!(
        slot.running().expect("the job holds the slot").kind,
        mcp::JobKind::Ingest
    );

    drop(open_the_pages);
    let ended = report_when_ended(&client, "job_status", &job_id).await;
    assert_eq!(ended["state"], "done", "{ended}");
    let released = async {
        while slot.running().is_some() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    };
    tokio::time::timeout(STAGE_WAIT, released)
        .await
        .expect("the slot should be free once the job has ended");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p mcp --test integration -- --ignored jobs::"]
async fn list_jobs_shows_the_running_job_and_the_recent_jobs_newest_first() {
    let slot = BackgroundJobs::new();
    let (services, open_the_pages) = StandInServices::new().with_page_gate();
    let (_stores, server) = empty_library("mcp-jobs-list", services, Some(slot.clone()));
    let client = connect(server).await;
    let sent = json!({ "media": MEDIA, "path": sample_pdf() });
    let first = call(&client, "ingest_pdf", sent.clone()).await;
    let first_id = structured(&first)["job_id"].as_str().unwrap().to_owned();
    status_in_stage(&client, &first_id, "converting").await;

    let listed = call(&client, "list_jobs", json!({})).await;

    let listed = structured(&listed);
    let file = sample_pdf()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert_eq!(listed["running"], json!({ "kind": "ingest", "name": file }));
    let jobs = listed["jobs"].as_array().unwrap();
    assert_eq!(jobs.len(), 1, "{listed}");
    assert_eq!(jobs[0]["job_id"], first_id.as_str());
    assert_eq!(jobs[0]["state"], "running");
    assert_eq!(jobs[0]["stage"]["name"], "converting");
    assert!(
        jobs[0].get("media").is_none(),
        "only the job view: {listed}"
    );

    drop(open_the_pages);
    report_when_ended(&client, "job_status", &first_id).await;
    let released = async {
        while structured(&call(&client, "list_jobs", json!({})).await)["running"] != Value::Null {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    };
    tokio::time::timeout(STAGE_WAIT, released)
        .await
        .expect("the slot should be free once the job has ended");

    // The PDF is stored now, so each of these jobs has ended when its call answers. Twenty-two
    // ingests are more than the list holds of one kind.
    let mut ingests = vec![first_id];
    for _ in 0..21 {
        let again = call(&client, "ingest_pdf", sent.clone()).await;
        ingests.push(structured(&again)["job_id"].as_str().unwrap().to_owned());
    }
    let insights = call(
        &client,
        "write_insights",
        json!({ "all": true, "dry_run": true }),
    )
    .await;
    let insights_id = structured(&insights)["job_id"].as_str().unwrap().to_owned();

    let listed = call(&client, "list_jobs", json!({})).await;

    let listed = structured(&listed);
    assert_eq!(listed["running"], Value::Null, "{listed}");
    let listed_ids: Vec<&str> = listed["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|job| job["job_id"].as_str().unwrap())
        .collect();
    let mut expected: Vec<&str> = vec![insights_id.as_str()];
    expected.extend(ingests.iter().rev().take(20).map(String::as_str));
    assert_eq!(listed_ids, expected);
}
