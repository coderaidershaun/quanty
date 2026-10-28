//! Checks the two ways a client reaches the server: streamable HTTP on a local port, and the
//! standard input and output of the built `quanty-mcp` command.

use std::process::Stdio;
use std::time::{Duration, Instant};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use mcp::{QuantyServer, http_service};
use ocr::testing::sample_pdf;
use rag_ingestion::testing::ThrowawayStores;
use rmcp::model::ProgressNotificationParam;
use rmcp::service::RunningService;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::{RoleClient, ServiceExt};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::process::Command;

use crate::support::{
    ClosedPorts, JOB_WAIT, ProgressRecorder, StandInServices, call, connect,
    connect_recording_progress, error_text, report_when_ended, status_in_stage, structured,
};

/// How long the command may take to end once the client has closed its input.
const END_WAIT: Duration = Duration::from_secs(20);

const TOOL_NAMES: [&str; 40] = [
    "catalog",
    "describe_tool",
    "guide",
    "list_media",
    "list_documents",
    "document_overview",
    "document_outline",
    "read_section",
    "read_pages",
    "read_item",
    "read_page",
    "chapter_contents",
    "chapter_insights",
    "search",
    "read_many",
    "answer",
    "find_concepts",
    "concept",
    "concept_neighbours",
    "concept_items",
    "concept_path",
    "compare_documents",
    "document_concepts",
    "chapter_concepts",
    "similar_items",
    "job_status",
    "watch_job",
    "list_jobs",
    "stop_job",
    "ingest_status",
    "insights_status",
    "dataset_status",
    "check_pdf",
    "ingest_pdf",
    "write_insights",
    "build_dataset",
    "health",
    "edit_media",
    "edit_document_tags",
    "export_insights",
];

#[tokio::test(flavor = "multi_thread")]
async fn the_tools_are_called_over_streamable_http_on_a_local_port() {
    let ports = ClosedPorts::new();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = http_service(ports.server());
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let transport = StreamableHttpClientTransport::from_uri(format!("http://{address}/mcp"));
    let client = ().serve(transport).await.unwrap();

    let tools = client.list_all_tools().await.unwrap();

    let names: Vec<&str> = tools.iter().map(|tool| tool.name.as_ref()).collect();
    assert_eq!(names, TOOL_NAMES);
    // 5 MiB of zero bytes is 6,990,508 characters of base64, over the 4 MiB that the HTTP server
    // takes unless it is told to take more. The PDF is refused for what it is, so the body got
    // through, and it is refused before anything is written.
    let big = STANDARD.encode(vec![0u8; 5 * 1024 * 1024]);
    let result = call(
        &client,
        "ingest_pdf",
        json!({ "book": "A Test Book", "pdf_base64": big, "file_name": "chapter-1-big.pdf" }),
    )
    .await;
    let text = error_text(&result);
    assert!(text.contains("not a PDF"), "{text}");
    assert!(
        !ports.config.content_folder.join("_uploads").exists(),
        "nothing was written"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_quanty_mcp_command_serves_the_tools_over_stdio() {
    let ports = ClosedPorts::new();
    let mut command = Command::new(env!("CARGO_BIN_EXE_quanty-mcp"));
    command
        .current_dir(ports.folder())
        .env("QDRANT_URL", &ports.config.qdrant_url)
        .env("FALKORDB_URL", &ports.config.falkordb_url)
        .env("FALKORDB_GRAPH", &ports.config.falkordb_graph)
        .env("QDRANT_ITEMS_COLLECTION", &ports.config.items_collection)
        .env(
            "QDRANT_CONCEPTS_COLLECTION",
            &ports.config.concepts_collection,
        )
        .env("CONCEPT_CACHE_DIR", &ports.config.concept_cache_folder)
        .env("CONCEPT_DECISION_LOG", &ports.config.concept_decision_log)
        .env("CONTENT_DIR", &ports.config.content_folder)
        // An empty value counts as not set, so a key that the shell exports does not reach the
        // command.
        .env("EMBEDDING_GEMINI_API_KEY", "")
        .env("CONVERTER_JEV_API_KEY", "")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().expect("quanty-mcp should start");
    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let logged = tokio::spawn(async move {
        let mut text = String::new();
        stderr.read_to_string(&mut text).await.unwrap();
        text
    });
    // The client skips a line that is not JSON, so a stray line on the standard output would not
    // stop it. The forwarder keeps every such line, to be looked at when the command has ended.
    let (from_command, to_client) = tokio::io::simplex(1 << 20);
    let strays = tokio::spawn(async move {
        let mut to_client = to_client;
        let mut lines = BufReader::new(stdout).lines();
        let mut strays = Vec::new();
        while let Some(line) = lines.next_line().await.unwrap() {
            if !serde_json::from_str::<Value>(&line).is_ok_and(|value| value.is_object()) {
                strays.push(line.clone());
            }
            to_client
                .write_all(format!("{line}\n").as_bytes())
                .await
                .unwrap();
        }
        strays
    });
    let client = ().serve((from_command, stdin)).await.unwrap();

    let tools = client.list_all_tools().await.unwrap();
    client.cancel().await.unwrap();

    let names: Vec<&str> = tools.iter().map(|tool| tool.name.as_ref()).collect();
    assert_eq!(names, TOOL_NAMES);
    let status = tokio::time::timeout(END_WAIT, child.wait())
        .await
        .expect("the command should end when its input is closed")
        .unwrap();
    assert!(status.success(), "{status}");
    assert_eq!(strays.await.unwrap(), Vec::<String>::new());
    let log = logged.await.unwrap();
    assert!(
        log.contains("serving over the standard input and output"),
        "the start line goes to the standard error: {log}"
    );
}

async fn progress_of(client: &RunningService<RoleClient, ProgressRecorder>, job_id: &str) -> f64 {
    let status = call(client, "job_status", json!({ "job_id": job_id })).await;
    structured(&status)["progress"].as_f64().unwrap()
}

/// What the first notification of a job that is held in `converting` says: no page of the 7 is
/// saved, so the job has done the 3 hundredths before the pages and 15 of 100 of the 60 hundredths
/// that the pages take.
fn assert_converting_progress(first: &ProgressNotificationParam) {
    assert_eq!(first.progress, 12.0, "{first:?}");
    assert_eq!(first.total, Some(100.0), "{first:?}");
    assert_eq!(
        first.message.as_deref(),
        Some("converting the pages: 0 of 7 done")
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p mcp --test integration -- --ignored wire::transports::"]
async fn watch_job_sends_progress_over_stdio_and_streamable_http() {
    let (services, open_the_pages) = StandInServices::new().with_page_gate();
    let stores = ThrowawayStores::new("mcp-progress");
    let server = QuantyServer::new(stores.config().clone(), services);
    let starter = connect(server.clone()).await;
    let started = call(
        &starter,
        "ingest_pdf",
        json!({ "media": "Option Volatility and Pricing", "path": sample_pdf() }),
    )
    .await;
    let job_id = structured(&started)["job_id"].as_str().unwrap().to_owned();
    status_in_stage(&starter, &job_id, "converting").await;
    let watch = json!({ "job_id": job_id, "timeout_s": 1 });

    // The pipe of the harness runs the same line-by-line transport as the standard input and output
    // of the `quanty-mcp` command, which cannot run a job without paid services.
    let (piped, piped_progress) = connect_recording_progress(server.clone()).await;
    let watched = call(&piped, "watch_job", watch.clone()).await;
    assert_eq!(structured(&watched)["stage"]["name"], "converting");
    assert_converting_progress(&piped_progress.first().await);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = http_service(server);
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let transport = StreamableHttpClientTransport::from_uri(format!("http://{address}/mcp"));
    let http_progress = ProgressRecorder::default();
    let over_http = http_progress.clone().serve(transport).await.unwrap();
    let watched = call(&over_http, "watch_job", watch).await;
    assert_eq!(structured(&watched)["stage"]["name"], "converting");
    assert_converting_progress(&http_progress.first().await);

    // Two pages are saved while one wait goes on, a tick apart, so the bar of that wait must rise.
    let mut sampled = vec![progress_of(&piped, &job_id).await];
    let seen_before = piped_progress.at_least(1).await.len();
    let (_, ()) = tokio::join!(
        call(
            &piped,
            "watch_job",
            json!({ "job_id": job_id, "timeout_s": 5 })
        ),
        async {
            tokio::time::sleep(Duration::from_millis(500)).await;
            open_the_pages.send(()).unwrap();
            tokio::time::sleep(Duration::from_millis(2500)).await;
            open_the_pages.send(()).unwrap();
        }
    );
    let during_pages = piped_progress.at_least(seen_before + 2).await;
    let this_wait = &during_pages[seen_before..];
    assert!(
        this_wait
            .iter()
            .all(|notice| notice.progress_token == this_wait[0].progress_token),
        "{this_wait:?}"
    );
    sampled.push(progress_of(&piped, &job_id).await);

    // The rest of the job is followed to its end, one wait for each stage.
    drop(open_the_pages);
    let followed = Instant::now();
    loop {
        let watched = call(
            &piped,
            "watch_job",
            json!({ "job_id": job_id, "timeout_s": 5 }),
        )
        .await;
        sampled.push(structured(&watched)["progress"].as_f64().unwrap());
        let status = call(&piped, "job_status", json!({ "job_id": job_id })).await;
        sampled.push(structured(&status)["progress"].as_f64().unwrap());
        if structured(&status)["state"] != "running" {
            break;
        }
        assert!(followed.elapsed() < JOB_WAIT, "the job did not end in time");
    }
    assert!(
        sampled.windows(2).all(|pair| pair[0] <= pair[1]),
        "{sampled:?}"
    );
    assert_eq!(sampled.last(), Some(&1.0), "{sampled:?}");
    let ended = report_when_ended(&starter, "job_status", &job_id).await;
    assert_eq!(ended["state"], "done", "{ended}");
    assert_eq!(ended["progress"], 1.0, "{ended}");

    // Each wait is one progress token, and its bar only rises.
    let all = piped_progress.at_least(during_pages.len()).await;
    let mut tokens = Vec::new();
    for notice in &all {
        if !tokens.contains(&notice.progress_token) {
            tokens.push(notice.progress_token.clone());
        }
    }
    for token in tokens {
        let values: Vec<f64> = all
            .iter()
            .filter(|notice| notice.progress_token == token)
            .map(|notice| notice.progress)
            .collect();
        assert!(
            values.windows(2).all(|pair| pair[0] < pair[1]),
            "{token:?}: {values:?}"
        );
    }
}
