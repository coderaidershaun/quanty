//! Checks `ingest_pdf` with `pdf_url`: a PDF on a local host is fetched, saved and ingested, and
//! every URL that is not allowed is refused without writing anything.

use mcp::QuantyServer;
use ocr::testing::sample_pdf;
use rag_core::Config;
use rag_ingestion::testing::ThrowawayStores;
use serde_json::{Value, json};

use crate::support::{
    ClosedPorts, PdfHost, Reply, StandInServices, call, connect, error_text, report_when_ended,
    sample_document_id, structured,
};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p mcp --test integration -- --ignored ingest::pdf_url::"]
async fn a_pdf_sent_by_url_is_saved_under_the_content_folder_and_ingested() {
    // The gate holds the conversion, so the first answer is `running` however fast the
    // stand-ins are.
    let (services, open_the_gate) = StandInServices::new().with_gate();
    let stores = ThrowawayStores::new("mcp-ingest-url");
    let client = connect(QuantyServer::new(stores.config().clone(), services)).await;
    let bytes = std::fs::read(sample_pdf()).unwrap();
    let host = PdfHost::start(vec![("/api/files/7", Reply::Ok(bytes.clone()))]).await;

    let first = call(
        &client,
        "ingest_pdf",
        json!({
            "media": "Option Volatility and Pricing",
            "pdf_url": host.url("/api/files/7"),
            "file_name": "chapter-1-sample-pages.pdf",
        }),
    )
    .await;

    let started = structured(&first);
    assert_eq!(started["state"], "running");
    assert_eq!(started["kind"], "ingest", "{started}");
    assert!(started["started_at"].is_string(), "{started}");
    let job_id = started["job_id"].as_str().unwrap();
    open_the_gate.send(()).unwrap();
    let ended = report_when_ended(&client, "job_status", job_id).await;
    assert_eq!(ended["state"], "done", "{ended}");
    assert_eq!(ended["document_id"], sample_document_id().to_string());
    assert!(ended["items"].as_u64().unwrap() > 0, "{ended}");
    let content = &stores.config().content_folder;
    let saved = content.join("_uploads/option-volatility-and-pricing/chapter-1-sample-pages.pdf");
    assert_eq!(std::fs::read(saved).unwrap(), bytes);
    assert!(
        content
            .join("option-volatility-and-pricing/chapter-1/chapter.json")
            .is_file()
    );
    assert_eq!(host.requested(), ["/api/files/7"]);
}

const LIMIT: u64 = 1024;

fn pdf_bytes(length: usize) -> Vec<u8> {
    let mut bytes = b"%PDF-".to_vec();
    bytes.resize(length, 0);
    bytes
}

/// The text of the refusal of `ingest_pdf` with these arguments added to the media, which never
/// points at the `health` tool because no service is the cause.
async fn refusal(client: &crate::support::Client, arguments: Value) -> String {
    let mut sent = json!({ "media": "A Test Book" });
    sent.as_object_mut()
        .unwrap()
        .extend(arguments.as_object().unwrap().clone());
    let text = error_text(&call(client, "ingest_pdf", sent).await);
    assert!(!text.contains("`health`"), "{text}");
    text
}

#[tokio::test(flavor = "multi_thread")]
async fn a_pdf_url_is_refused_before_anything_is_written() {
    let ports = ClosedPorts::new();
    let config = Config {
        fetch_origins: vec!["https://jet.example".parse().unwrap()],
        ..ports.config.clone()
    };
    let server = QuantyServer::with_max_pdf_bytes(config, StandInServices::new(), LIMIT);
    let client = connect(server).await;
    let host = PdfHost::start(vec![
        ("/moved", Reply::Redirect("/elsewhere.pdf".to_owned())),
        ("/not-a-pdf", Reply::Ok(b"plain text, not a PDF".to_vec())),
        ("/too-big", Reply::Ok(pdf_bytes(2000))),
        ("/endless", Reply::Endless),
        ("/elsewhere.pdf", Reply::Ok(pdf_bytes(200))),
    ])
    .await;
    let name = "chapter-1-fetched.pdf";
    let fetch = |path: &str| json!({ "pdf_url": host.url(path), "file_name": name });

    let text = refusal(&client, json!({ "pdf_url": host.url("/too-big") })).await;
    assert!(
        text.contains("`pdf_url` was given without `file_name`"),
        "{text}"
    );
    let text = refusal(
        &client,
        json!({ "path": "/a/chapter-1-a.pdf", "pdf_url": host.url("/too-big"), "file_name": name }),
    )
    .await;
    assert!(text.contains("both `path` and `pdf_url`"), "{text}");
    let text = refusal(
        &client,
        json!({ "pdf_url": "not a url", "file_name": name }),
    )
    .await;
    assert!(text.contains("is not a URL"), "{text}");
    let ftp = format!("ftp://127.0.0.1:{}/chapter.pdf", host.port());
    let text = refusal(&client, json!({ "pdf_url": ftp, "file_name": name })).await;
    assert!(text.contains("http or https"), "{text}");

    for refused in [
        "http://192.0.2.1/x.pdf",
        "http://localhost.evil.example/x.pdf",
        "http://127.0.0.1@evil.example/x.pdf",
        "https://jet.example.evil.test/x.pdf",
        "http://jet.example/x.pdf",
        "https://jet.example:8443/x.pdf",
    ] {
        let text = refusal(&client, json!({ "pdf_url": refused, "file_name": name })).await;
        assert!(text.contains("not an allowed origin"), "{refused}: {text}");
        assert!(text.contains("QUANTY_FETCH_ORIGINS"), "{refused}: {text}");
    }
    assert!(host.requested().is_empty(), "nothing was fetched yet");

    let text = refusal(&client, fetch("/moved")).await;
    assert!(text.contains("302"), "{text}");
    assert!(text.contains("/elsewhere.pdf"), "{text}");
    assert!(text.contains("redirects are not followed"), "{text}");
    assert_eq!(host.requested(), ["/moved"], "the redirect is not followed");

    let text = refusal(&client, fetch("/missing")).await;
    assert!(text.contains("404"), "{text}");
    let text = refusal(&client, fetch("/not-a-pdf")).await;
    assert!(text.contains("not a PDF"), "{text}");
    let text = refusal(&client, fetch("/too-big")).await;
    assert!(text.contains("over the size limit of 1024 bytes"), "{text}");
    let text = refusal(&client, fetch("/endless")).await;
    assert!(text.contains("over the size limit of 1024 bytes"), "{text}");

    assert!(
        !ports.config.content_folder.exists(),
        "a refused PDF is never saved"
    );
    assert_eq!(
        host.requested(),
        ["/moved", "/missing", "/not-a-pdf", "/too-big", "/endless"]
    );
}
