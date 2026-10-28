//! Checks `ingest_pdf` and `ingest_status`: a PDF is ingested, a failed job says why, an old job is
//! forgotten, and a bad file is refused before anything is written.

use std::path::PathBuf;
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use mcp::QuantyServer;
use ocr::testing::{Scenario, sample_pdf};
use rag_ingestion::testing::ThrowawayStores;
use rmcp::model::{ServerNotification, SubscriptionFilter};
use serde_json::json;

use crate::support::{
    ClosedPorts, StandInServices, call, connect, connect_on_2026_07_28,
    connect_recording_list_changes, error_text, report_when_ended, sample_document_id, structured,
};

fn pdf_bytes(length: usize) -> Vec<u8> {
    let mut bytes = b"%PDF-".to_vec();
    bytes.resize(length, 0);
    bytes
}

/// The files the test sends by path are outside the folder of the server, so that nothing that a
/// refusal could have written is mistaken for them.
#[tokio::test(flavor = "multi_thread")]
async fn a_bad_file_is_refused_before_anything_is_written() {
    const LIMIT: u64 = 1024;
    let ports = ClosedPorts::new();
    let client = connect(ports.server_with_limit(LIMIT)).await;
    let sent = tempfile::tempdir().unwrap();
    let file = |name: &str, bytes: &[u8]| -> PathBuf {
        let path = sent.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    };
    let plain_text = file("chapter-1-text.pdf", b"This is plain text, not a PDF.");
    let too_big = file("chapter-2-big.pdf", &pdf_bytes(2000));
    let refused_by_path = |path: &str| {
        call(
            &client,
            "ingest_pdf",
            json!({ "book": "A Test Book", "path": path }),
        )
    };
    let refused_as_base64 = |bytes: &[u8], name: &str| {
        call(
            &client,
            "ingest_pdf",
            json!({ "book": "A Test Book", "pdf_base64": STANDARD.encode(bytes), "file_name": name }),
        )
    };

    let text = error_text(&refused_by_path(plain_text.to_str().unwrap()).await);
    assert!(text.contains("not a PDF"), "{text}");
    assert!(text.contains("chapter-1-text.pdf"), "{text}");
    let text = error_text(&refused_by_path(too_big.to_str().unwrap()).await);
    assert!(text.contains("over the size limit of 1024 bytes"), "{text}");
    let text = error_text(&refused_by_path("chapter-1-text.pdf").await);
    assert!(text.contains("absolute"), "{text}");
    let gone = sent.path().join("chapter-3-gone.pdf");
    let text = error_text(&refused_by_path(gone.to_str().unwrap()).await);
    assert!(text.contains("could not read the PDF"), "{text}");
    assert!(text.contains("the machine the server runs on"), "{text}");
    let text = error_text(&refused_by_path(sent.path().to_str().unwrap()).await);
    assert!(text.contains("not a file"), "{text}");

    let text = error_text(&refused_as_base64(b"plain text again", "chapter-1-text.pdf").await);
    assert!(text.contains("not a PDF"), "{text}");
    let text = error_text(&refused_as_base64(&pdf_bytes(2000), "chapter-2-big.pdf").await);
    assert!(text.contains("over the size limit of 1024 bytes"), "{text}");
    let not_base64 = call(
        &client,
        "ingest_pdf",
        json!({ "book": "A Test Book", "pdf_base64": "!!! not base64 !!!", "file_name": "chapter-1-a.pdf" }),
    )
    .await;
    let text = error_text(&not_base64);
    assert!(text.contains("not standard base64"), "{text}");
    assert!(
        !ports.config.content_folder.exists(),
        "a refused PDF is never saved"
    );

    let pdf = STANDARD.encode(pdf_bytes(200));

    for name in [
        "../chapter-1-escape.pdf",
        "sub/chapter-1-escape.pdf",
        "/tmp/chapter-1-escape.pdf",
        "..",
    ] {
        let result = call(
            &client,
            "ingest_pdf",
            json!({ "book": "A Test Book", "pdf_base64": pdf, "file_name": name }),
        )
        .await;

        let text = error_text(&result);
        assert!(text.contains("plain file name"), "{name}: {text}");
    }

    let left_in_folder = std::fs::read_dir(ports.folder()).unwrap().count();
    assert_eq!(left_in_folder, 0, "nothing was written");
}

/// Stand-in services over empty throwaway stores, whose content folder is where the chapter is
/// converted.
fn empty_library(
    test_name: &str,
    services: StandInServices,
) -> (ThrowawayStores, QuantyServer<StandInServices>) {
    let stores = ThrowawayStores::new(test_name);
    let server = QuantyServer::new(stores.config().clone(), services);
    (stores, server)
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p mcp --test integration -- --ignored ingest::"]
async fn a_pdf_sent_by_path_is_ingested_and_a_second_send_is_already_ingested() {
    // The gate holds the conversion, so the first answer is `running` however fast the
    // stand-ins are.
    let (services, open_the_gate) = StandInServices::new().with_gate();
    let (stores, server) = empty_library("mcp-ingest-path", services);
    let client = connect(server.clone()).await;
    // One listener of each protocol, to be told that the list of resources changed when the job
    // ends.
    let (_session, session_notices) = connect_recording_list_changes(server.clone()).await;
    let listener = connect_on_2026_07_28(server).await;
    let mut stream = listener
        .listen(
            SubscriptionFilter::builder()
                .resources_list_changed()
                .build(),
        )
        .await
        .expect("the server should accept a listener of the list of resources");
    // A paper's PDF can have any name.
    let plain = stores.temporary_folder().join("hawkes-notes.pdf");
    std::fs::copy(sample_pdf(), &plain).unwrap();
    let sent = json!({
        "media": "Hawkes Processes in Finance",
        "category": "paper",
        "path": plain,
        "authors": ["A. Author", " B. Author ", ""],
        "tags": ["Hawkes"],
        "document_tags": ["Options", "volatility"],
    });
    // The library does not have the media yet, so with no category it is a book, whose PDF must
    // be named as a chapter.
    let as_a_book = json!({ "media": "Hawkes Processes in Finance", "path": plain });
    let text = error_text(&call(&client, "ingest_pdf", as_a_book).await);
    assert!(text.contains("chapter-<number>-<name>.pdf"), "{text}");
    assert!(text.contains("rename"), "{text}");
    assert!(text.contains("document_title"), "{text}");

    let first = call(&client, "ingest_pdf", sent.clone()).await;

    let started = structured(&first);
    assert_eq!(started["state"], "running");
    assert_eq!(started["media"], "Hawkes Processes in Finance");
    assert_eq!(started["file"], "hawkes-notes.pdf");
    assert_eq!(started["kind"], "ingest", "{started}");
    assert!(started["started_at"].is_string(), "{started}");
    assert!(started["updated_at"].is_string(), "{started}");
    let job_id = started["job_id"].as_str().unwrap();
    open_the_gate.send(()).unwrap();
    let ended = report_when_ended(&client, "ingest_status", job_id).await;
    let notice = tokio::time::timeout(Duration::from_secs(10), stream.next())
        .await
        .expect("the listener should be told that the list of resources changed");
    assert!(
        matches!(
            notice,
            Ok(Some(ServerNotification::ResourceListChangedNotification(_)))
        ),
        "{notice:?}"
    );
    session_notices.first().await;
    assert_eq!(ended["state"], "done", "{ended}");
    assert_eq!(ended["kind"], "ingest", "{ended}");
    assert!(ended["started_at"].is_string(), "{ended}");
    assert!(ended["updated_at"].is_string(), "{ended}");
    assert_eq!(ended["document_id"], sample_document_id().to_string());
    assert!(ended["items"].as_u64().unwrap() > 0);
    assert!(ended.get("stage").is_none());
    let summary = ended["summary"].as_str().unwrap();
    assert!(summary.contains("pages: 7"), "{summary}");
    let used = ended["usage"]
        .as_array()
        .expect("a done job says what it used");
    assert!(!used.is_empty());
    for model in used {
        for field in [
            "model",
            "input_tokens",
            "output_tokens",
            "cache_read_tokens",
            "cache_write_tokens",
            "estimated",
        ] {
            assert!(model.get(field).is_some(), "{field} is not in {model}");
        }
    }
    assert!(ended["cost_usd"].is_number(), "{ended}");
    let documents = call(&client, "list_documents", json!({})).await;
    assert_eq!(
        structured(&documents)["documents"],
        json!([{
            "document_id": sample_document_id().to_string(),
            "title": "Hawkes Processes in Finance",
            "media": "Hawkes Processes in Finance",
            "category": "paper",
            "authors": ["A. Author", "B. Author"],
            "media_tags": ["hawkes"],
            "tags": ["options", "volatility"],
            "pages": 7,
            "ingested_whole": true,
            "items": { "chunk": 6, "formula": 6, "figure": 6, "table": 5, "code": 0 },
        }])
    );

    // The stored media is a paper, so a send that gives no category, and a title that only a
    // paper's PDF takes, is named as before.
    let mut sent_again = sent;
    sent_again["category"] = json!(null);
    sent_again["document_title"] = json!("Hawkes Processes in Finance");
    let second = call(&client, "ingest_pdf", sent_again.clone()).await;

    let again = structured(&second);
    assert_eq!(again["state"], "already_ingested", "{again}");
    assert_eq!(again["document_id"], ended["document_id"]);
    assert_eq!(again["items"], ended["items"]);
    assert_ne!(again["job_id"], started["job_id"], "a job of its own");
    assert!(
        again.get("usage").is_none() && again.get("cost_usd").is_none(),
        "a document that was already ingested used nothing: {again}"
    );

    // A chapter that was ingested before insights were written has no file. Sending it again
    // writes that file and nothing else, and the report says what that used.
    let catalogue = ocr::Catalogue::read(&stores.config().content_folder).unwrap();
    let chapter = &catalogue.chapters[0].folder;
    std::fs::remove_file(chapter.join("insights.json")).unwrap();

    let third = call(&client, "ingest_pdf", sent_again.clone()).await;

    let third_job = structured(&third)["job_id"].as_str().unwrap();
    let writing = report_when_ended(&client, "ingest_status", third_job).await;
    assert_eq!(writing["state"], "already_ingested", "{writing}");
    assert_eq!(writing["items"], ended["items"]);
    let summary = writing["summary"].as_str().unwrap();
    assert!(summary.contains("insights: written"), "{summary}");
    assert!(
        writing["usage"]
            .as_array()
            .is_some_and(|used| !used.is_empty()),
        "{writing}"
    );
    assert!(writing["cost_usd"].is_number(), "{writing}");
    assert!(chapter.join("insights.json").is_file());
    std::fs::remove_file(chapter.join("artifacts.json")).unwrap();
    let fourth = call(&client, "ingest_pdf", sent_again).await;
    let repaired = report_when_ended(
        &client,
        "ingest_status",
        structured(&fourth)["job_id"].as_str().unwrap(),
    )
    .await;
    assert_eq!(repaired["state"], "already_ingested", "{repaired}");
    assert_eq!(repaired["artifacts_done"], 17);
    assert_eq!(repaired["artifacts_total"], 17);
    assert!(
        repaired["summary"]
            .as_str()
            .unwrap()
            .contains("artifacts: 17 complete, 17 written")
    );
    assert!(
        repaired["usage"]
            .as_array()
            .is_some_and(|used| !used.is_empty())
    );
    assert!(chapter.join("artifacts.json").is_file());
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p mcp --test integration -- --ignored ingest::"]
async fn a_pdf_sent_as_base64_is_saved_under_the_content_folder_and_ingested() {
    // The gate holds the conversion, so the first answer is `running` however fast the
    // stand-ins are.
    let (services, open_the_gate) = StandInServices::new().with_gate();
    let (stores, server) = empty_library("mcp-ingest-base64", services);
    let client = connect(server).await;
    let bytes = std::fs::read(sample_pdf()).unwrap();

    let saved = stores
        .config()
        .content_folder
        .join("_uploads/option-volatility-and-pricing/chapter-1-sample-pages.pdf");
    std::fs::create_dir_all(saved.parent().unwrap()).unwrap();
    std::fs::write(&saved, b"%PDF-an-older-upload").unwrap();

    let first = call(
        &client,
        "ingest_pdf",
        json!({
            "book": "Option Volatility and Pricing",
            "pdf_base64": STANDARD.encode(&bytes),
            "file_name": "chapter-1-sample-pages.pdf",
        }),
    )
    .await;

    let started = structured(&first);
    assert_eq!(started["state"], "running");
    open_the_gate.send(()).unwrap();
    let ended = report_when_ended(
        &client,
        "ingest_status",
        started["job_id"].as_str().unwrap(),
    )
    .await;
    assert_eq!(ended["state"], "done", "{ended}");
    assert_eq!(ended["document_id"], sample_document_id().to_string());
    let content = &stores.config().content_folder;
    let saved = content.join("_uploads/option-volatility-and-pricing/chapter-1-sample-pages.pdf");
    assert_eq!(std::fs::read(saved).unwrap(), bytes);
    assert!(
        content
            .join("option-volatility-and-pricing/chapter-1/chapter.json")
            .is_file()
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p mcp --test integration -- --ignored ingest::"]
async fn a_job_that_fails_after_the_paid_work_began_says_failed_and_what_to_do() {
    // Every reply for page 1 fails the check, so each conversion stops after its paid calls.
    let (services, open_the_gate) = StandInServices::new()
        .with_pages(Scenario::AllTables {
            broken_page: Some(1),
        })
        .with_gate();
    let (_stores, server) = empty_library("mcp-ingest-failed", services);
    let client = connect(server).await;
    let sent = json!({ "book": "Option Volatility and Pricing", "path": sample_pdf() });

    let first = call(&client, "ingest_pdf", sent.clone()).await;

    let started = structured(&first);
    assert_eq!(started["state"], "running");
    let job_id = started["job_id"].as_str().unwrap();
    open_the_gate.send(()).unwrap();
    let ended = report_when_ended(&client, "ingest_status", job_id).await;
    assert_eq!(ended["state"], "failed", "{ended}");
    assert!(ended.get("stage").is_none(), "{ended}");
    let error = ended["error"].as_str().unwrap();
    assert!(error.contains("page 1"), "{error}");
    assert!(
        !error.contains("`health`"),
        "no service is down when a reply is rejected: {error}"
    );
    assert!(
        error.ends_with(
            "Send the same PDF again to go on: converted pages and kept answers are not paid for twice."
        ),
        "{error}"
    );

    // The gate holds the second conversion too, so this answer is `running` and not the failure
    // of a job that already ended.
    let second = call(&client, "ingest_pdf", sent).await;

    let again = structured(&second);
    assert_eq!(again["state"], "running", "{again}");
    assert_ne!(again["job_id"], started["job_id"], "a job of its own");
    open_the_gate.send(()).unwrap();
    let ended_again =
        report_when_ended(&client, "ingest_status", again["job_id"].as_str().unwrap()).await;
    assert_eq!(ended_again["state"], "failed", "{ended_again}");

    a_job_that_a_file_stops_does_not_point_at_health().await;
}

async fn a_job_that_a_file_stops_does_not_point_at_health() {
    let (services, open_the_gate) = StandInServices::new().with_gate();
    let (stores, server) = empty_library("mcp-ingest-file", services);
    // A file is where the folder of kept answers must be made, so the ingest stops there, after
    // every store has answered.
    let in_the_way = &stores.config().concept_cache_folder;
    std::fs::write(in_the_way, "not a folder").unwrap();
    let client = connect(server).await;
    let sent = json!({ "book": "Option Volatility and Pricing", "path": sample_pdf() });

    let first = call(&client, "ingest_pdf", sent).await;

    let started = structured(&first);
    assert_eq!(started["state"], "running");
    open_the_gate.send(()).unwrap();
    let ended = report_when_ended(
        &client,
        "ingest_status",
        started["job_id"].as_str().unwrap(),
    )
    .await;
    assert_eq!(ended["state"], "failed", "{ended}");
    let error = ended["error"].as_str().unwrap();
    assert!(error.contains(&in_the_way.display().to_string()), "{error}");
    assert!(
        !error.contains("`health`"),
        "no service is down when a file is in the way: {error}"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p mcp --test integration -- --ignored ingest::"]
async fn a_job_is_forgotten_when_a_hundred_newer_jobs_were_started() {
    let (_stores, server) = empty_library("mcp-ingest-forgotten", StandInServices::new());
    let client = connect(server).await;
    let sent = json!({ "book": "Option Volatility and Pricing", "path": sample_pdf() });
    let first = call(&client, "ingest_pdf", sent.clone()).await;
    let first_id = structured(&first)["job_id"].as_str().unwrap().to_owned();
    let ended = report_when_ended(&client, "ingest_status", &first_id).await;
    assert_eq!(ended["state"], "done", "{ended}");
    // Each of these jobs finds the PDF in the stores, so it has ended when its call answers.
    let send_again = async || {
        let again = call(&client, "ingest_pdf", sent.clone()).await;
        let report = structured(&again).clone();
        assert_eq!(report["state"], "already_ingested", "{report}");
        report["job_id"].as_str().unwrap().to_owned()
    };
    for _ in 0..99 {
        send_again().await;
    }
    let status = call(&client, "ingest_status", json!({ "job_id": first_id })).await;
    assert_eq!(
        structured(&status)["state"],
        "done",
        "the first of a hundred jobs is still kept"
    );

    let newest_id = send_again().await;

    let forgotten = call(&client, "ingest_status", json!({ "job_id": first_id })).await;
    let text = error_text(&forgotten);
    assert!(text.contains(&first_id), "{text}");
    assert!(text.contains("last 100 jobs"), "{text}");
    let status = call(&client, "ingest_status", json!({ "job_id": newest_id })).await;
    assert_eq!(structured(&status)["state"], "already_ingested");
}
