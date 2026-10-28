//! Checks what an agent sees before it calls anything: the list of tools with their input
//! schemas.

use rmcp::model::{CacheScope, ProtocolVersion, Tool};
use serde_json::{Value, json};

use super::marks::{
    assert_concept_tools_are_marked_and_have_schemas, assert_job_tools_are_marked_and_have_schemas,
    assert_library_tools_are_marked_and_have_schemas,
};
use crate::support::{ClosedPorts, connect, connect_on_2026_07_28};

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
    let expected_names = [
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
    assert_eq!(names, expected_names);
    let expected: [(&str, &[&str], &[&str]); 40] = [
        ("catalog", &["detail", "group"], &[]),
        ("describe_tool", &["name"], &["name"]),
        ("guide", &["topic"], &["topic"]),
        ("list_media", &["author", "category", "detail", "tag"], &[]),
        (
            "list_documents",
            &[
                "author", "category", "cursor", "detail", "limit", "media", "tags",
            ],
            &[],
        ),
        (
            "document_overview",
            &["detail", "document_id"],
            &["document_id"],
        ),
        ("document_outline", &["document_id"], &["document_id"]),
        (
            "read_section",
            &["as", "cursor", "document_id", "max_chars", "section"],
            &["document_id", "section"],
        ),
        (
            "read_pages",
            &["as", "cursor", "document_id", "from", "max_chars", "to"],
            &["document_id", "from", "to"],
        ),
        (
            "read_item",
            &[
                "cursor",
                "document_id",
                "item_id",
                "kind",
                "label",
                "max_chars",
            ],
            &[],
        ),
        (
            "read_page",
            &["cursor", "document_id", "max_chars", "page"],
            &["document_id", "page"],
        ),
        (
            "chapter_contents",
            &["cursor", "document_id", "kind", "max_chars"],
            &["document_id"],
        ),
        (
            "chapter_insights",
            &["cursor", "document_id", "max_chars"],
            &["document_id"],
        ),
        (
            "search",
            &[
                "author",
                "category",
                "cursor",
                "detail",
                "document",
                "explain",
                "kind",
                "limit",
                "max_chars",
                "media",
                "offset",
                "question",
                "section",
                "tags",
            ],
            &["question"],
        ),
        ("read_many", &["max_chars", "requests"], &["requests"]),
        (
            "answer",
            &[
                "author", "category", "document", "kind", "media", "question", "tags",
            ],
            &["question"],
        ),
        ("find_concepts", &["limit", "name", "query"], &[]),
        ("concept", &["concept_id", "name"], &[]),
        (
            "concept_neighbours",
            &["concept_id", "depth", "direction", "kinds", "limit"],
            &["concept_id"],
        ),
        (
            "concept_items",
            &["concept_id", "cursor", "document", "kind", "limit"],
            &["concept_id"],
        ),
        (
            "concept_path",
            &["from", "max_depth", "to"],
            &["from", "to"],
        ),
        (
            "compare_documents",
            &["document_ids", "limit"],
            &["document_ids"],
        ),
        (
            "document_concepts",
            &["document_id", "limit", "page", "with_counts"],
            &["document_id"],
        ),
        (
            "chapter_concepts",
            &["cursor", "document_id", "max_chars", "page"],
            &["document_id"],
        ),
        ("similar_items", &["item_id", "kind", "limit"], &["item_id"]),
        ("job_status", &["job_id"], &["job_id"]),
        ("watch_job", &["job_id", "timeout_s"], &["job_id"]),
        ("list_jobs", &[], &[]),
        ("stop_job", &["job_id"], &["job_id"]),
        ("ingest_status", &["job_id"], &["job_id"]),
        ("insights_status", &["job_id"], &["job_id"]),
        ("dataset_status", &["job_id"], &["job_id"]),
        (
            "check_pdf",
            &[
                "category",
                "document_title",
                "file_name",
                "media",
                "path",
                "pdf_url",
            ],
            &["media"],
        ),
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
                "pdf_url",
                "tags",
            ],
            &["media"],
        ),
        (
            "write_insights",
            &["all", "document_id", "dry_run", "media"],
            &[],
        ),
        (
            "build_dataset",
            &[
                "documents",
                "dry_run",
                "holdout",
                "include_book_parts",
                "keep_unrendered",
                "max_sft",
                "max_synthetic",
                "media",
                "min_page_score",
                "name",
                "questions",
                "rewrites",
                "run_code",
                "samples",
            ],
            &["name"],
        ),
        ("health", &[], &[]),
        (
            "edit_media",
            &["authors", "category", "media", "tags"],
            &["media"],
        ),
        (
            "edit_document_tags",
            &["add", "document_id", "remove"],
            &["document_id"],
        ),
        (
            "export_insights",
            &["document_id", "format", "overwrite", "path"],
            &["document_id", "format", "path"],
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
    let phrases = [
        ("answer", "subscription"),
        ("ingest_pdf", "PAID"),
        ("build_dataset", "PAID"),
        ("build_dataset", "dry_run"),
        ("write_insights", "PAID"),
        ("write_insights", "dry_run"),
        ("edit_media", "both stores"),
        ("export_insights", "file"),
        ("export_insights", "overwrite"),
        ("stop_job", "an ingest, an insights job or a dataset build"),
    ];
    for (name, phrase) in phrases {
        assert!(description(name).contains(phrase), "{}", description(name));
    }

    for old in ["ingest_status", "insights_status", "dataset_status"] {
        assert!(
            description(old).starts_with("Deprecated: call `job_status`."),
            "{}",
            description(old)
        );
    }
    assert_job_tools_are_marked_and_have_schemas(&tools);
    assert_library_tools_are_marked_and_have_schemas(&tools);
    assert_concept_tools_are_marked_and_have_schemas(&tools);

    let peer = client
        .peer_info()
        .expect("the handshake gave the server info");
    assert_eq!(peer.protocol_version, ProtocolVersion::V_2025_11_25);
    let listed = client.list_tools(None).await.unwrap();
    assert_eq!(listed.ttl_ms, None, "a 2025-11-25 peer gets no cache hint");
    assert_eq!(listed.cache_scope, None);

    let health = tools.iter().find(|tool| tool.name == "health").unwrap();
    let output_schema = health
        .output_schema
        .as_ref()
        .expect("health declares its output schema");
    let mut result_fields: Vec<&str> = output_schema["properties"]
        .as_object()
        .expect("the output schema has properties")
        .keys()
        .map(String::as_str)
        .collect();
    result_fields.sort();
    assert_eq!(result_fields, ["healthy", "keys", "models", "report"]);
    assert_eq!(
        serde_json::to_value(&health.meta).unwrap(),
        json!({ "quanty/group": "admin", "quanty/cost": { "paid": false } })
    );

    // The rmcp client keeps a list that has a time to live, so each read needs its own connection.
    let mut names_by_read = Vec::new();
    for _ in 0..2 {
        let client = connect_on_2026_07_28(ports.server()).await;
        let peer = client
            .peer_info()
            .expect("the discovery gave the server info");
        assert_eq!(peer.protocol_version, ProtocolVersion::V_2026_07_28);
        let listed = client.list_tools(None).await.unwrap();
        assert_eq!(listed.ttl_ms, Some(300_000));
        assert_eq!(listed.cache_scope, Some(CacheScope::Public));
        let listed_names: Vec<String> = listed
            .tools
            .iter()
            .map(|tool| tool.name.to_string())
            .collect();
        names_by_read.push(listed_names);
    }
    assert_eq!(names_by_read[0], names_by_read[1]);
    assert_eq!(names_by_read[0], expected_names);
}
