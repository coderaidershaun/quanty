//! Checks that a failure reaches the agent as a tool error whose text says what to do: a store
//! that is down, a model that is signed out, a bad argument, a job or a document that is not
//! there, and a chapter that cannot be read.

use mcp::QuantyServer;
use ocr::testing::sample_pdf;
use rag_core::{ConceptId, DocId, ItemId, ItemKind};
use rag_ingestion::testing::StandInLlm;
use serde_json::{Value, json};

use crate::support::{
    Client, ClosedPorts, StandInServices, call, connect, error_text, sample_content_folder,
    sample_document_id, sample_notes_document_id,
};

const STORE_DOWN: [&str; 2] = ["falkor://127.0.0.1:1", "Call the `health` tool"];

/// Calls a tool that must fail, checks that its error holds each of `words`, and gives the error.
async fn refused(client: &Client, tool: &'static str, arguments: Value, words: &[&str]) -> String {
    let text = error_text(&call(client, tool, arguments.clone()).await);
    for word in words {
        assert!(text.contains(word), "{tool} {arguments}: {text}");
    }
    text
}

/// The stores are down, so an argument or a choice that is checked after the first connection
/// would be hidden behind the error of the store.
#[tokio::test(flavor = "multi_thread")]
async fn every_refusal_is_a_tool_error_that_says_what_to_do() {
    let ports = ClosedPorts::new();
    let client = connect(ports.server()).await;
    let id = sample_document_id().to_string();
    let a_book = "A Book";
    let calls: Vec<(&'static str, Value, Vec<&str>)> = vec![
        (
            "search",
            json!({ "question": "what is delta?" }),
            STORE_DOWN.to_vec(),
        ),
        (
            "ingest_pdf",
            json!({ "book": "Option Volatility and Pricing", "path": sample_pdf() }),
            [&STORE_DOWN[..], &["Send the same PDF again"]].concat(),
        ),
        (
            "build_dataset",
            json!({ "name": "t", "dry_run": true }),
            [&STORE_DOWN[..], &["could not connect to the graph"]].concat(),
        ),
        (
            "search",
            json!({ "question": "what is delta?", "kind": "poem" }),
            vec!["`poem`", "chunk, formula, figure, table"],
        ),
        (
            "search",
            json!({ "kind": "chunk" }),
            vec!["missing field `question`"],
        ),
        (
            "answer",
            json!({ "question": "   " }),
            vec!["question is blank"],
        ),
        (
            "read_page",
            json!({ "document_id": "not-an-id", "page": 1 }),
            vec!["`document_id`", "not-an-id"],
        ),
        (
            "ingest_status",
            json!({ "job_id": "no-such-job" }),
            vec!["no-such-job", "send the PDF again"],
        ),
        (
            "dataset_status",
            json!({ "job_id": "no-such-job" }),
            vec!["no-such-job", "`build_dataset`"],
        ),
        (
            "build_dataset",
            json!({ "name": "  " }),
            vec!["`name` is blank"],
        ),
        (
            "ingest_pdf",
            json!({ "book": a_book, "path": "/a/chapter-1-a.pdf", "pdf_base64": "AAAA" }),
            vec!["both `path` and `pdf_base64`"],
        ),
        (
            "stop_job",
            json!({ "job_id": "no-such-job" }),
            vec![
                "no-such-job",
                "job ids are forgotten when the server restarts",
            ],
        ),
        (
            "insights_status",
            json!({ "job_id": "no-such-job" }),
            vec!["no-such-job", "call `write_insights` again"],
        ),
        (
            "write_insights",
            json!({ "document_id": "not-an-id" }),
            vec!["`document_id`", "not-an-id"],
        ),
        (
            "write_insights",
            json!({ "all": true }),
            STORE_DOWN.to_vec(),
        ),
        (
            "write_insights",
            json!({ "media": a_book, "dry_run": true }),
            STORE_DOWN.to_vec(),
        ),
        (
            "write_insights",
            json!({ "document_id": id }),
            STORE_DOWN.to_vec(),
        ),
        (
            "edit_media",
            json!({ "media": a_book, "tags": [] }),
            STORE_DOWN.to_vec(),
        ),
        (
            "edit_document_tags",
            json!({ "document_id": id, "add": ["a"] }),
            STORE_DOWN.to_vec(),
        ),
        (
            "find_concepts",
            json!({ "query": "how much a price moves" }),
            STORE_DOWN.to_vec(),
        ),
        ("concept", json!({}), STORE_DOWN.to_vec()),
        (
            "compare_documents",
            json!({ "document_ids": [id, sample_notes_document_id(1).to_string()] }),
            STORE_DOWN.to_vec(),
        ),
        (
            "similar_items",
            json!({ "item_id": ItemId::new(sample_document_id(), ItemKind::Chunk, 0).to_string() }),
            vec!["http://127.0.0.1:1", "Call the `health` tool"],
        ),
    ];
    for (tool, arguments, words) in calls {
        refused(&client, tool, arguments, &words).await;
    }
    for choice in [
        json!({}),
        json!({ "all": false }),
        json!({ "media": "  " }),
        json!({ "document_id": id, "media": a_book }),
        json!({ "media": a_book, "all": true }),
        json!({ "document_id": id, "all": true }),
    ] {
        let words = ["give exactly one of `document_id`, `media` and `all: true`"];
        refused(&client, "write_insights", choice, &words).await;
    }
    for (tool, arguments, words) in [
        (
            "edit_media",
            json!({ "media": a_book }),
            "nothing to change",
        ),
        (
            "edit_media",
            json!({ "media": a_book, "tags": ["good", " "] }),
            "a tag in `tags` is blank",
        ),
        (
            "edit_media",
            json!({ "media": a_book, "category": "poem" }),
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
        (
            "concept_neighbours",
            json!({ "concept_id": ConceptId::random().to_string(), "depth": 3 }),
            "`depth` is 3",
        ),
        (
            "concept_path",
            json!({ "from": "delta", "to": "gamma", "max_depth": 5 }),
            "`max_depth` is 5",
        ),
        (
            "find_concepts",
            json!({}),
            "exactly one of `name` and `query`",
        ),
        (
            "compare_documents",
            json!({ "document_ids": [id] }),
            "from 2 to 10 different document ids",
        ),
    ] {
        let text = refused(&client, tool, arguments, &[words]).await;
        assert!(!text.contains("127.0.0.1"), "no store was reached: {text}");
    }

    a_model_that_is_signed_out_stops_answer_before_anything_is_searched(&ports).await;
    a_page_or_a_document_that_is_not_there_points_at_list_documents().await;
    a_chapter_that_cannot_be_read_is_named_when_a_document_is_not_found().await;
}

/// A call that got as far as the search would name a store, and the graph is connected before the
/// embedder is paid.
async fn a_model_that_is_signed_out_stops_answer_before_anything_is_searched(ports: &ClosedPorts) {
    let signed_out = StandInLlm::signed_out();
    let services = StandInServices::new().with_llm(signed_out.clone());
    let client = connect(QuantyServer::new(ports.config.clone(), services)).await;

    let words = [
        "could not write an answer",
        "claude is not signed in",
        "Call the `health` tool",
    ];
    let text = refused(
        &client,
        "answer",
        json!({ "question": "what is delta?" }),
        &words,
    )
    .await;
    assert!(!text.contains("127.0.0.1"), "{text}");
    assert_eq!(signed_out.calls(), 0, "no question is asked");

    let search = json!({ "question": "what is delta?" });
    // A search needs no model, so it goes on to the stores.
    refused(&client, "search", search, &["falkor://127.0.0.1:1"]).await;
}

/// Only these calls need a chapter to read, so only this server reads the committed samples: no
/// call that could write is sent to it.
async fn a_page_or_a_document_that_is_not_there_points_at_list_documents() {
    let samples = ClosedPorts::new().with_content_folder(&sample_content_folder());
    let reader = connect(samples.server()).await;
    let page_99 = json!({ "document_id": sample_document_id().to_string(), "page": 99 });
    refused(&reader, "read_page", page_99, &["page 99", "7 pages"]).await;

    let unknown = DocId::from_source_sha256("a pdf that was never converted").to_string();
    let content = sample_content_folder().display().to_string();
    let words = [unknown.as_str(), content.as_str(), "`list_documents`"];
    let asked = json!({ "document_id": unknown, "page": 1 });
    refused(&reader, "read_page", asked, &words).await;
}

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

    let path = broken.display().to_string();
    let words = [asked.as_str(), "cannot be read", path.as_str()];
    let arguments = json!({ "document_id": asked, "page": 1 });
    refused(&client, "read_page", arguments, &words).await;
}
