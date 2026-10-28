//! Checks `check_pdf`: it says what an `ingest_pdf` of the same PDF would do, for a new PDF, one
//! sent by URL, one that is stored already and one whose name another PDF holds, and it writes
//! nothing.

use mcp::QuantyServer;
use ocr::testing::sample_pdf;
use rag_ingestion::testing::ThrowawayStores;
use serde_json::{Value, json};

use crate::support::{
    PdfHost, Reply, StandInServices, call, connect, report_when_ended, sample_document_id,
    structured,
};

const MEDIA: &str = "Option Volatility and Pricing";
const FILE: &str = "chapter-1-sample-pages.pdf";

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p mcp --test integration -- --ignored check_pdf::"]
async fn check_pdf_says_what_an_ingest_would_do_and_writes_nothing() {
    let stores = ThrowawayStores::new("mcp-check-pdf");
    let client = connect(QuantyServer::new(
        stores.config().clone(),
        StandInServices::new(),
    ))
    .await;
    let bytes = std::fs::read(sample_pdf()).unwrap();
    let path = stores.temporary_folder().join(FILE);
    std::fs::write(&path, &bytes).unwrap();
    let content = stores.config().content_folder.clone();

    let new = checked(&client, json!({ "media": MEDIA, "path": path })).await;

    assert_eq!(new["state"], "new", "{new}");
    assert_eq!(new["pages"], 7, "{new}");
    assert_eq!(new["blockers"], json!([]), "{new}");
    assert_eq!(new["document_id"], sample_document_id().to_string());
    assert_eq!(new["media"], MEDIA);
    assert_eq!(new["category"], "book");
    assert_eq!(new["file"], FILE);
    let estimate = &new["estimate"];
    assert_eq!(estimate["pages_to_convert"], 7, "{new}");
    assert!(estimate["usd_low"].as_f64().unwrap() > 0.0, "{new}");
    assert!(
        estimate["usd_high"].as_f64().unwrap() >= estimate["usd_low"].as_f64().unwrap(),
        "{new}"
    );
    assert!(estimate["minutes"].as_u64().unwrap() > 0, "{new}");
    let title = new["title"].as_str().unwrap().to_owned();

    let host = PdfHost::start(vec![("/api/files/7", Reply::Ok(bytes))]).await;
    let by_url = checked(
        &client,
        json!({ "media": MEDIA, "pdf_url": host.url("/api/files/7"), "file_name": FILE }),
    )
    .await;

    for field in ["state", "pages", "title", "estimate", "document_id"] {
        assert_eq!(by_url[field], new[field], "{field} by URL: {by_url}");
    }
    assert_eq!(host.requested(), ["/api/files/7"]);
    assert!(
        !content.exists() || std::fs::read_dir(&content).unwrap().next().is_none(),
        "a check writes nothing under the content folder"
    );
    let jobs = call(&client, "list_jobs", json!({})).await;
    assert_eq!(structured(&jobs)["jobs"], json!([]));

    let started = call(
        &client,
        "ingest_pdf",
        json!({ "media": MEDIA, "path": path }),
    )
    .await;
    let job_id = structured(&started)["job_id"].as_str().unwrap().to_owned();
    let ended = report_when_ended(&client, "job_status", &job_id).await;
    assert_eq!(ended["state"], "done", "{ended}");
    let listed = call(&client, "list_documents", json!({})).await;
    assert_eq!(structured(&listed)["documents"][0]["title"], title);

    let ingested = checked(&client, json!({ "media": MEDIA, "path": path })).await;

    assert_eq!(ingested["state"], "ingested", "{ingested}");
    assert!(ingested["items"].as_u64().unwrap() > 0, "{ingested}");
    assert_eq!(ingested["pages"], Value::Null, "{ingested}");
    assert_eq!(ingested["blockers"], json!([]), "{ingested}");
    assert_eq!(ingested["estimate"]["pages_to_convert"], 0, "{ingested}");
    assert_eq!(ingested["estimate"]["minutes"], 0, "{ingested}");

    let other = stores.temporary_folder().join("chapter-1-other.pdf");
    std::fs::write(&other, b"%PDF-1.4 a different file").unwrap();
    let saved = std::fs::read(content.join("option-volatility-and-pricing/chapter-1/chapter.json"))
        .unwrap();

    let clash = checked(&client, json!({ "media": MEDIA, "path": other })).await;

    assert_eq!(clash["state"], Value::Null, "{clash}");
    let blockers = clash["blockers"].as_array().unwrap();
    assert!(!blockers.is_empty(), "{clash}");
    assert!(
        blockers[0].as_str().unwrap().contains("different"),
        "{clash}"
    );
    assert_eq!(
        std::fs::read(content.join("option-volatility-and-pricing/chapter-1/chapter.json"))
            .unwrap(),
        saved,
        "a check leaves the chapter folder as it was"
    );
}

async fn checked(client: &crate::support::Client, arguments: Value) -> Value {
    structured(&call(client, "check_pdf", arguments).await).clone()
}
