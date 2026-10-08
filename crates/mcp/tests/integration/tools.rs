//! Checks what an agent sees before it calls anything: the list of tools with their input
//! schemas, and the report of `health`.

use rmcp::model::Tool;
use serde_json::{Value, json};

use crate::support::{ClosedPorts, call, connect, structured};

fn property_names(tool: &Tool) -> Vec<String> {
    assert_eq!(
        tool.input_schema.get("type"),
        Some(&json!("object")),
        "the schema of {} is an object",
        tool.name
    );
    let mut names: Vec<String> = tool
        .input_schema
        .get("properties")
        .and_then(Value::as_object)
        .map(|properties| properties.keys().cloned().collect())
        .unwrap_or_default();
    names.sort();
    names
}

/// A tool with no required argument has no `required` key at all.
fn required_names(tool: &Tool) -> Vec<String> {
    let mut names: Vec<String> = tool
        .input_schema
        .get("required")
        .and_then(Value::as_array)
        .map(|required| {
            required
                .iter()
                .map(|name| name.as_str().expect("a name is text").to_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[tokio::test(flavor = "multi_thread")]
async fn the_server_lists_every_tool_with_its_input_schema() {
    let ports = ClosedPorts::new();
    let client = connect(ports.server()).await;

    let tools = client.list_all_tools().await.unwrap();

    let names: Vec<&str> = tools.iter().map(|tool| tool.name.as_ref()).collect();
    assert_eq!(
        names,
        [
            "answer",
            "health",
            "ingest_pdf",
            "ingest_status",
            "list_documents",
            "read_page",
            "search"
        ]
    );
    let expected: [(&str, &[&str], &[&str]); 7] = [
        (
            "answer",
            &["author", "category", "kind", "media", "question", "tags"],
            &["question"],
        ),
        ("health", &[], &[]),
        (
            "ingest_pdf",
            &[
                "authors",
                "category",
                "document_tags",
                "document_title",
                "file_name",
                "media",
                "path",
                "pdf_base64",
                "tags",
            ],
            &["media"],
        ),
        ("ingest_status", &["job_id"], &["job_id"]),
        ("list_documents", &[], &[]),
        (
            "read_page",
            &["document_id", "page"],
            &["document_id", "page"],
        ),
        (
            "search",
            &[
                "author", "category", "explain", "kind", "limit", "media", "question", "tags",
            ],
            &["question"],
        ),
    ];
    for (tool, (name, properties, required)) in tools.iter().zip(expected) {
        assert_eq!(property_names(tool), properties, "the arguments of {name}");
        assert_eq!(
            required_names(tool),
            required,
            "the required arguments of {name}"
        );
    }
    let description = |name: &str| {
        tools
            .iter()
            .find(|tool| tool.name == name)
            .and_then(|tool| tool.description.as_deref())
            .unwrap_or_else(|| panic!("{name} has a description"))
    };
    assert!(
        description("answer").contains("subscription"),
        "{}",
        description("answer")
    );
    assert!(
        description("ingest_pdf").contains("PAID"),
        "{}",
        description("ingest_pdf")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn health_names_each_service_that_is_not_ready() {
    let ports = ClosedPorts::new();
    let client = connect(ports.server()).await;

    let result = call(&client, "health", json!({})).await;

    let report = structured(&result);
    assert_eq!(report["healthy"], false);
    let lines = report["report"].as_str().unwrap();
    assert!(lines.contains("Qdrant: FAILED"), "{lines}");
    assert!(lines.contains("FalkorDB: FAILED"), "{lines}");
    assert!(lines.contains("claude:"), "{lines}");
}
