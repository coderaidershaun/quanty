//! Checks that a failure reaches the agent as a tool error whose text says what to do: a store
//! that is down, a bad argument, a document that is not there, and a chapter that cannot be read.

use mcp::QuantyServer;
use ocr::testing::sample_pdf;
use rag_core::{DocId, EmbedError};
use rag_ingestion::testing::StandInLlm;
use serde_json::json;

use crate::support::{
    ClosedPorts, Library, StandInServices, call, connect, error_text, sample_content_folder,
    sample_document_id,
};

#[tokio::test(flavor = "multi_thread")]
async fn a_store_that_is_down_is_a_tool_error_that_points_at_health() {
    let ports = ClosedPorts::new();
    let client = connect(ports.server()).await;

    let search = call(&client, "search", json!({ "question": "what is delta?" })).await;

    let text = error_text(&search);
    assert!(text.contains("falkor://127.0.0.1:1"), "{text}");
    assert!(text.contains("Call the `health` tool"), "{text}");
    let started = call(
        &client,
        "ingest_pdf",
        json!({ "book": "Option Volatility and Pricing", "path": sample_pdf() }),
    )
    .await;
    let text = error_text(&started);
    assert!(text.contains("falkor://127.0.0.1:1"), "{text}");
    assert!(text.contains("Call the `health` tool"), "{text}");
    assert!(text.contains("Send the same PDF again"), "{text}");
    let build = call(
        &client,
        "build_dataset",
        json!({ "name": "t", "dry_run": true }),
    )
    .await;
    let text = error_text(&build);
    assert!(text.contains("could not connect to the graph"), "{text}");
    assert!(text.contains("falkor://127.0.0.1:1"), "{text}");
    assert!(text.contains("Call the `health` tool"), "{text}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_model_that_is_signed_out_stops_answer_before_anything_is_searched() {
    // The stores are down, so a call that got as far as the search would name a store, and the
    // graph is connected before the embedder is paid.
    let ports = ClosedPorts::new();
    let signed_out = StandInLlm::signed_out();
    let services = StandInServices::new().with_llm(signed_out.clone());
    let client = connect(QuantyServer::new(ports.config.clone(), services)).await;

    let answer = call(&client, "answer", json!({ "question": "what is delta?" })).await;

    let text = error_text(&answer);
    assert!(text.contains("could not write an answer"), "{text}");
    assert!(text.contains("claude is not signed in"), "{text}");
    assert!(text.contains("Call the `health` tool"), "{text}");
    assert!(!text.contains("127.0.0.1"), "{text}");
    assert_eq!(signed_out.calls(), 0, "no question is asked");

    let search = call(&client, "search", json!({ "question": "what is delta?" })).await;
    let text = error_text(&search);
    assert!(
        text.contains("falkor://127.0.0.1:1"),
        "a search needs no model, so it goes on to the stores: {text}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bad_argument_is_a_tool_error_that_says_what_is_allowed() {
    // The stores are down, so an argument that is checked after the first connection would be
    // hidden behind the error of the store.
    let ports = ClosedPorts::new();
    let client = connect(ports.server()).await;

    let kind = call(
        &client,
        "search",
        json!({ "question": "what is delta?", "kind": "poem" }),
    )
    .await;
    let text = error_text(&kind);
    assert!(text.contains("`poem`"), "{text}");
    assert!(text.contains("chunk, formula, figure, table"), "{text}");

    let no_question = call(&client, "search", json!({ "kind": "chunk" })).await;
    let text = error_text(&no_question);
    assert!(text.contains("missing field `question`"), "{text}");

    let blank = call(&client, "answer", json!({ "question": "   " })).await;
    let text = error_text(&blank);
    assert!(text.contains("question is blank"), "{text}");

    let not_an_id = call(
        &client,
        "read_page",
        json!({ "document_id": "not-an-id", "page": 1 }),
    )
    .await;
    let text = error_text(&not_an_id);
    assert!(text.contains("`document_id`"), "{text}");
    assert!(text.contains("not-an-id"), "{text}");

    // Only this call needs a chapter to read, so only this server reads the committed samples:
    // no call that could write is sent to it.
    let samples = ClosedPorts::new().with_content_folder(&sample_content_folder());
    let reader = connect(samples.server()).await;
    let page_99 = call(
        &reader,
        "read_page",
        json!({ "document_id": sample_document_id().to_string(), "page": 99 }),
    )
    .await;
    let text = error_text(&page_99);
    assert!(text.contains("page 99"), "{text}");
    assert!(text.contains("7 pages"), "{text}");

    let unknown_job = call(&client, "ingest_status", json!({ "job_id": "no-such-job" })).await;
    let text = error_text(&unknown_job);
    assert!(text.contains("no-such-job"), "{text}");
    assert!(text.contains("send the PDF again"), "{text}");

    let unknown_build = call(
        &client,
        "dataset_status",
        json!({ "job_id": "no-such-job" }),
    )
    .await;
    let text = error_text(&unknown_build);
    assert!(text.contains("no-such-job"), "{text}");
    assert!(text.contains("`build_dataset`"), "{text}");

    let no_name = call(&client, "build_dataset", json!({ "name": "  " })).await;
    let text = error_text(&no_name);
    assert!(text.contains("`name` is blank"), "{text}");

    let both = call(
        &client,
        "ingest_pdf",
        json!({ "book": "A Book", "path": "/a/chapter-1-a.pdf", "pdf_base64": "AAAA" }),
    )
    .await;
    let text = error_text(&both);
    assert!(text.contains("both `path` and `pdf_base64`"), "{text}");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unknown_document_is_a_tool_error_that_points_at_list_documents() {
    let ports = ClosedPorts::new().with_content_folder(&sample_content_folder());
    let client = connect(ports.server()).await;
    let unknown = DocId::from_source_sha256("a pdf that was never converted").to_string();

    let result = call(
        &client,
        "read_page",
        json!({ "document_id": unknown, "page": 1 }),
    )
    .await;

    let text = error_text(&result);
    assert!(text.contains(&unknown), "{text}");
    assert!(
        text.contains(&sample_content_folder().display().to_string()),
        "{text}"
    );
    assert!(text.contains("`list_documents`"), "{text}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_chapter_that_cannot_be_read_is_named_when_a_document_is_not_found() {
    let ports = ClosedPorts::new();
    let broken = ports
        .config
        .content_folder
        .join("a-book/chapter-1/chapter.json");
    std::fs::create_dir_all(broken.parent().unwrap()).unwrap();
    std::fs::write(&broken, "not json").unwrap();
    let client = connect(ports.server()).await;
    let asked = DocId::from_source_sha256("the pdf of the broken chapter").to_string();

    let result = call(
        &client,
        "read_page",
        json!({ "document_id": asked, "page": 1 }),
    )
    .await;

    let text = error_text(&result);
    assert!(text.contains(&asked), "{text}");
    assert!(text.contains("cannot be read"), "{text}");
    assert!(text.contains(&broken.display().to_string()), "{text}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_job_that_this_server_does_not_have_is_named() {
    let ports = ClosedPorts::new();
    let client = connect(ports.server()).await;

    let stop = call(&client, "stop_job", json!({ "job_id": "no-such-job" })).await;

    let text = error_text(&stop);
    assert!(text.contains("no-such-job"), "{text}");
    assert!(
        text.contains("job ids are forgotten when the server restarts"),
        "{text}"
    );
    let status = call(
        &client,
        "insights_status",
        json!({ "job_id": "no-such-job" }),
    )
    .await;
    let text = error_text(&status);
    assert!(text.contains("no-such-job"), "{text}");
    assert!(text.contains("call `write_insights` again"), "{text}");
}

#[tokio::test(flavor = "multi_thread")]
async fn write_insights_takes_exactly_one_choice() {
    // The stores are down, so a choice that is checked after the first connection would be hidden
    // behind the error of the store.
    let ports = ClosedPorts::new();
    let client = connect(ports.server()).await;
    let id = sample_document_id().to_string();

    for choice in [
        json!({}),
        json!({ "all": false }),
        json!({ "media": "  " }),
        json!({ "document_id": id, "media": "A Book" }),
        json!({ "media": "A Book", "all": true }),
        json!({ "document_id": id, "all": true }),
    ] {
        let result = call(&client, "write_insights", choice.clone()).await;

        let text = error_text(&result);
        assert!(
            text.contains("give exactly one of `document_id`, `media` and `all: true`"),
            "{choice}: {text}"
        );
    }
    let bad_id = call(
        &client,
        "write_insights",
        json!({ "document_id": "not-an-id" }),
    )
    .await;
    let text = error_text(&bad_id);
    assert!(text.contains("`document_id`"), "{text}");
    assert!(text.contains("not-an-id"), "{text}");
    for choice in [
        json!({ "all": true }),
        json!({ "media": "A Book", "dry_run": true }),
        json!({ "document_id": id }),
    ] {
        let result = call(&client, "write_insights", choice.clone()).await;

        let text = error_text(&result);
        assert!(text.contains("falkor://127.0.0.1:1"), "{choice}: {text}");
        assert!(text.contains("Call the `health` tool"), "{choice}: {text}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_library_edit_that_is_not_valid_is_refused_before_any_store() {
    // The stores are down, so an edit that is checked after the first connection would be hidden
    // behind the error of the store.
    let ports = ClosedPorts::new();
    let client = connect(ports.server()).await;
    let id = sample_document_id().to_string();

    for (tool, arguments, words) in [
        (
            "edit_media",
            json!({ "media": "A Book" }),
            "nothing to change",
        ),
        (
            "edit_media",
            json!({ "media": "A Book", "tags": ["good", " "] }),
            "a tag in `tags` is blank",
        ),
        (
            "edit_media",
            json!({ "media": "A Book", "category": "poem" }),
            "`category` is not valid",
        ),
        (
            "edit_media",
            json!({ "media": "  ", "tags": [] }),
            "`media` is blank",
        ),
        (
            "edit_document_tags",
            json!({ "document_id": "not-an-id", "add": ["a"] }),
            "not-an-id",
        ),
        (
            "edit_document_tags",
            json!({ "document_id": id }),
            "nothing to change",
        ),
        (
            "edit_document_tags",
            json!({ "document_id": id, "remove": [" "] }),
            "a tag in `add` or `remove` is blank",
        ),
    ] {
        let result = call(&client, tool, arguments.clone()).await;

        let text = error_text(&result);
        assert!(text.contains(words), "{tool} {arguments}: {text}");
        assert!(!text.contains("127.0.0.1"), "no store was reached: {text}");
    }
    for (tool, arguments) in [
        ("edit_media", json!({ "media": "A Book", "tags": [] })),
        (
            "edit_document_tags",
            json!({ "document_id": id, "add": ["a"] }),
        ),
    ] {
        let result = call(&client, tool, arguments).await;

        let text = error_text(&result);
        assert!(text.contains("falkor://127.0.0.1:1"), "{tool}: {text}");
        assert!(text.contains("Call the `health` tool"), "{tool}: {text}");
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p mcp --test integration -- --ignored failures::"]
async fn a_spent_credit_is_named_with_its_billing_page() {
    let library = Library::ingested("mcp-credit").await;
    let rejected = |status: u16, body: &'static str| {
        let services = StandInServices::new().with_failing_embedder(move || EmbedError::Rejected {
            status,
            body: body.to_owned(),
        });
        library.server_over(&sample_content_folder(), services)
    };
    let client = connect(rejected(402, "Your prepayment credits are depleted")).await;

    let search = call(&client, "search", json!({ "question": "what is delta?" })).await;

    let text = error_text(&search);
    assert!(
        text.contains(
            " Your Google Gemini prepaid credit is used up: add credit at https://aistudio.google.com/billing, then try again. Call the `health` tool"
        ),
        "{text}"
    );
    assert!(
        text.ends_with("Call the `health` tool to see which service is not ready."),
        "{text}"
    );

    let client = connect(rejected(500, "the service is busy")).await;

    let search = call(&client, "search", json!({ "question": "what is delta?" })).await;

    let text = error_text(&search);
    assert!(text.contains("500"), "{text}");
    assert!(!text.contains("prepaid credit"), "{text}");
}
