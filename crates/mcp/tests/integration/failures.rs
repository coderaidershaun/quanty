//! Checks that a failure reaches the agent as a tool error whose text says what to do: a store
//! that is down, a bad argument, and a document that is not there.

use ocr::testing::sample_pdf;
use rag_core::DocId;
use serde_json::json;

use crate::support::{
    ClosedPorts, call, connect, error_text, sample_content_folder, sample_document_id,
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
