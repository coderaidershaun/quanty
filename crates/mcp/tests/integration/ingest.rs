//! Checks `ingest_pdf` and `ingest_status`: a PDF is ingested, one job runs at a time, a failed
//! job says why, an old job is forgotten, and a bad file is refused before anything is written.

use std::path::PathBuf;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use mcp::QuantyServer;
use ocr::testing::{Scenario, sample_pdf};
use rag_ingestion::testing::ThrowawayStores;
use serde_json::json;

use crate::support::{
    ClosedPorts, StandInServices, call, connect, error_text, report_when_ended, sample_document_id,
    structured,
};

fn pdf_bytes(length: usize) -> Vec<u8> {
    let mut bytes = b"%PDF-".to_vec();
    bytes.resize(length, 0);
    bytes
}

#[tokio::test(flavor = "multi_thread")]
async fn a_file_that_is_not_a_pdf_or_is_over_the_size_limit_is_refused() {
    const LIMIT: u64 = 1024;
    let ports = ClosedPorts::new();
    let client = connect(ports.server_with_limit(LIMIT)).await;
    let file = |name: &str, bytes: &[u8]| -> PathBuf {
        let path = ports.folder().join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    };
    let plain_text = file("chapter-1-text.pdf", b"This is plain text, not a PDF.");
    let too_big = file("chapter-2-big.pdf", &pdf_bytes(2000));
    let wrong_name = file("notes.pdf", &pdf_bytes(100));
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
    let text = error_text(&refused_by_path(wrong_name.to_str().unwrap()).await);
    assert!(text.contains("chapter-<number>-<name>.pdf"), "{text}");
    assert!(text.contains("rename"), "{text}");
    let text = error_text(&refused_by_path("chapter-1-text.pdf").await);
    assert!(text.contains("absolute"), "{text}");
    let gone = ports.folder().join("chapter-3-gone.pdf");
    let text = error_text(&refused_by_path(gone.to_str().unwrap()).await);
    assert!(text.contains("could not read the PDF"), "{text}");
    assert!(text.contains("the machine the server runs on"), "{text}");
    let text = error_text(&refused_by_path(ports.folder().to_str().unwrap()).await);
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
}

#[tokio::test(flavor = "multi_thread")]
async fn an_upload_whose_name_holds_a_folder_is_refused_and_nothing_is_written() {
    let ports = ClosedPorts::new();
    let client = connect(ports.server()).await;
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
    let (_stores, server) = empty_library("mcp-ingest-path", services);
    let client = connect(server).await;
    let sent = json!({
        "book": "Option Volatility and Pricing",
        "path": sample_pdf(),
        "author": "Sheldon Natenberg",
        "tags": ["Options", "volatility"],
    });

    let first = call(&client, "ingest_pdf", sent.clone()).await;

    let started = structured(&first);
    assert_eq!(started["state"], "running");
    assert_eq!(started["file"], "chapter-1-sample-pages.pdf");
    let job_id = started["job_id"].as_str().unwrap();
    open_the_gate.send(()).unwrap();
    let ended = report_when_ended(&client, job_id).await;
    assert_eq!(ended["state"], "done", "{ended}");
    assert_eq!(ended["document_id"], sample_document_id().to_string());
    assert!(ended["items"].as_u64().unwrap() > 0);
    assert!(ended.get("stage").is_none());
    let summary = ended["summary"].as_str().unwrap();
    assert!(summary.contains("pages: 7"), "{summary}");
    let documents = call(&client, "list_documents", json!({})).await;
    let document = &structured(&documents)["documents"][0];
    assert_eq!(document["document_id"], sample_document_id().to_string());
    assert_eq!(document["author"], "Sheldon Natenberg");
    assert_eq!(document["tags"], json!(["options", "volatility"]));
    assert_eq!(document["pages"], 7);

    let second = call(&client, "ingest_pdf", sent).await;

    let again = structured(&second);
    assert_eq!(again["state"], "already_ingested", "{again}");
    assert_eq!(again["document_id"], ended["document_id"]);
    assert_eq!(again["items"], ended["items"]);
    assert_ne!(again["job_id"], started["job_id"], "a job of its own");
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
    let ended = report_when_ended(&client, started["job_id"].as_str().unwrap()).await;
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
async fn a_second_pdf_is_refused_while_one_is_being_ingested() {
    let (services, open_the_gate) = StandInServices::new().with_gate();
    let (_stores, server) = empty_library("mcp-ingest-busy", services);
    let client = connect(server).await;
    let sent = json!({ "book": "Option Volatility and Pricing", "path": sample_pdf() });

    let first = call(&client, "ingest_pdf", sent.clone()).await;

    let started = structured(&first);
    assert_eq!(started["state"], "running");
    assert_eq!(started["stage"], "converting");
    let job_id = started["job_id"].as_str().unwrap();
    let second = call(&client, "ingest_pdf", sent).await;
    let text = error_text(&second);
    assert!(text.contains(job_id), "{text}");
    assert!(text.contains("ingest_status"), "{text}");
    let status = call(&client, "ingest_status", json!({ "job_id": job_id })).await;
    assert_eq!(structured(&status)["state"], "running");

    open_the_gate.send(()).unwrap();

    let ended = report_when_ended(&client, job_id).await;
    assert_eq!(ended["state"], "done", "{ended}");
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
    let ended = report_when_ended(&client, job_id).await;
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
    let ended_again = report_when_ended(&client, again["job_id"].as_str().unwrap()).await;
    assert_eq!(ended_again["state"], "failed", "{ended_again}");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p mcp --test integration -- --ignored ingest::"]
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
    let ended = report_when_ended(&client, started["job_id"].as_str().unwrap()).await;
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
    let ended = report_when_ended(&client, &first_id).await;
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
