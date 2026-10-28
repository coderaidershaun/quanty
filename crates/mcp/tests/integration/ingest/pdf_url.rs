//! Checks `ingest_pdf` with `pdf_url`: a PDF on a local host is fetched, saved and ingested, and
//! every URL that is not allowed is refused without writing anything.

use std::io;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

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

#[tokio::test(flavor = "multi_thread")]
async fn a_pdf_url_host_is_resolved_once_and_an_internal_address_is_refused() {
    let ports = ClosedPorts::new();
    // The host listens on 127.0.0.2, where the lookup of the system never leads for a name.
    let host = PdfHost::start_on("127.0.0.2", vec![("/x.pdf", Reply::Ok(pdf_bytes(200)))]).await;
    let port = host.port();
    let config = Config {
        fetch_origins: [
            format!("http://files.example:{port}"),
            format!("http://[::ffff:127.0.0.2]:{port}"),
        ]
        .map(|origin| origin.parse().unwrap())
        .to_vec(),
        ..ports.config.clone()
    };
    let lookups = Arc::new(AtomicUsize::new(0));
    // `None` is a lookup that fails.
    let answer: Arc<Mutex<Option<IpAddr>>> = Arc::new(Mutex::new(None));
    let services = StandInServices::new().with_resolver({
        let (lookups, answer) = (Arc::clone(&lookups), Arc::clone(&answer));
        move |_, port| {
            lookups.fetch_add(1, Ordering::SeqCst);
            let address = *answer.lock().unwrap();
            address
                .map(|ip| vec![SocketAddr::new(ip, port)])
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no such host"))
        }
    });
    let client = connect(QuantyServer::with_max_pdf_bytes(config, services, LIMIT)).await;
    let named = |name: &str| {
        json!({
            "media": "A Test Book",
            "pdf_url": format!("http://{name}:{port}/x.pdf"),
            "file_name": "chapter-1-named.pdf",
        })
    };
    // The PDF is fetched, and the call then stops at the stores, which are closed.
    let assert_fetched = |text: &str| {
        for refused in [
            "resolves to",
            "could not find the address",
            "could not fetch",
        ] {
            assert!(!text.contains(refused), "{text}");
        }
    };

    for internal in [
        "10.0.0.1",
        "172.16.0.9",
        "192.168.1.1",
        "169.254.169.254",
        "100.64.0.1",
        "0.0.0.0",
        "127.0.0.1",
        "::",
        "::1",
        "fd00::1",
        "fe80::1",
        "::ffff:10.0.0.1",
        "::ffff:127.0.0.1",
    ] {
        *answer.lock().unwrap() = Some(internal.parse().unwrap());
        let before = lookups.load(Ordering::SeqCst);
        let text = refusal(&client, named("files.example")).await;
        assert!(text.contains("files.example"), "{internal}: {text}");
        assert!(
            text.contains(&format!("resolves to {internal}")),
            "{internal}: {text}"
        );
        assert!(text.contains("QUANTY_FETCH_ORIGINS"), "{internal}: {text}");
        assert_eq!(lookups.load(Ordering::SeqCst), before + 1, "{internal}");
    }
    *answer.lock().unwrap() = None;
    let text = refusal(&client, named("files.example")).await;
    assert!(
        text.contains("could not find the address of files.example"),
        "{text}"
    );
    assert!(text.contains("no such host"), "{text}");
    assert!(host.requested().is_empty(), "nothing was connected to yet");

    // `localhost` is the one name that may lead to this machine, and the fetch goes to the
    // address that was checked: the system would send `localhost` to 127.0.0.1, where nothing
    // listens, and not to the host.
    *answer.lock().unwrap() = Some("127.0.0.2".parse().unwrap());
    let before = lookups.load(Ordering::SeqCst);
    let text = error_text(&call(&client, "ingest_pdf", named("localhost")).await);
    assert_eq!(lookups.load(Ordering::SeqCst), before + 1, "{text}");
    assert_eq!(host.requested(), ["/x.pdf"], "{text}");
    assert_fetched(&text);
    *answer.lock().unwrap() = Some("10.0.0.1".parse().unwrap());
    let text = refusal(&client, named("localhost")).await;
    assert!(text.contains("resolves to 10.0.0.1"), "{text}");

    // Whoever runs the server may list an IP address that is inside a private range, here the
    // loopback address in its IPv4-mapped form, which is not the loopback address of the rule.
    let before = lookups.load(Ordering::SeqCst);
    let text = error_text(&call(&client, "ingest_pdf", named("[::ffff:127.0.0.2]")).await);
    assert_eq!(
        lookups.load(Ordering::SeqCst),
        before,
        "an IP address is not looked up"
    );
    assert_eq!(host.requested(), ["/x.pdf", "/x.pdf"], "{text}");
    assert_fetched(&text);
}
