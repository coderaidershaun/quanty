//! Checks what an agent sees before it calls anything: the list of tools with their input
//! schemas, and the report of `health`.

use rmcp::model::Tool;
use serde_json::{Value, json};

use mcp::QuantyServer;
use rag_ingestion::{KeyReadFailure, KeyState, KeyStates};

use crate::support::{ClosedPorts, StandInServices, call, connect, structured};

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
            "build_dataset",
            "chapter_concepts",
            "chapter_contents",
            "chapter_insights",
            "dataset_status",
            "edit_document_tags",
            "edit_media",
            "export_insights",
            "health",
            "ingest_pdf",
            "ingest_status",
            "insights_status",
            "list_documents",
            "read_page",
            "search",
            "stop_job",
            "write_insights"
        ]
    );
    let expected: [(&str, &[&str], &[&str]); 18] = [
        (
            "answer",
            &[
                "author", "category", "document", "kind", "media", "question", "tags",
            ],
            &["question"],
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
        (
            "chapter_concepts",
            &["document_id", "page"],
            &["document_id"],
        ),
        (
            "chapter_contents",
            &["document_id", "kind"],
            &["document_id"],
        ),
        ("chapter_insights", &["document_id"], &["document_id"]),
        ("dataset_status", &["job_id"], &["job_id"]),
        (
            "edit_document_tags",
            &["add", "document_id", "remove"],
            &["document_id"],
        ),
        (
            "edit_media",
            &["authors", "category", "media", "tags"],
            &["media"],
        ),
        (
            "export_insights",
            &["document_id", "format", "overwrite", "path"],
            &["document_id", "format", "path"],
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
        ("insights_status", &["job_id"], &["job_id"]),
        ("list_documents", &[], &[]),
        (
            "read_page",
            &["document_id", "page"],
            &["document_id", "page"],
        ),
        (
            "search",
            &[
                "author", "category", "document", "explain", "kind", "limit", "media", "question",
                "tags",
            ],
            &["question"],
        ),
        ("stop_job", &["job_id"], &["job_id"]),
        (
            "write_insights",
            &["all", "document_id", "dry_run", "media"],
            &[],
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
    assert!(
        description("build_dataset").contains("PAID"),
        "{}",
        description("build_dataset")
    );
    assert!(
        description("build_dataset").contains("dry_run"),
        "{}",
        description("build_dataset")
    );
    assert!(
        description("write_insights").contains("PAID"),
        "{}",
        description("write_insights")
    );
    assert!(
        description("write_insights").contains("dry_run"),
        "{}",
        description("write_insights")
    );
    assert!(
        description("edit_media").contains("both stores"),
        "{}",
        description("edit_media")
    );
    assert!(
        description("export_insights").contains("file"),
        "{}",
        description("export_insights")
    );
    assert!(
        description("export_insights").contains("overwrite"),
        "{}",
        description("export_insights")
    );
    assert!(
        description("stop_job").contains("an ingest, an insights job or a dataset build"),
        "{}",
        description("stop_job")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn health_names_each_service_that_is_not_ready_and_says_why_a_key_cannot_be_used() {
    let ports = ClosedPorts::new();
    let client = connect(ports.server()).await;

    let result = call(&client, "health", json!({})).await;

    let report = structured(&result);
    assert_eq!(report["healthy"], false);
    let lines = report["report"].as_str().unwrap();
    assert!(lines.contains("Qdrant: FAILED"), "{lines}");
    assert!(lines.contains("FalkorDB: FAILED"), "{lines}");
    assert!(lines.contains("agent:"), "{lines}");
    assert!(lines.contains("Gemini key: ok (accepted)"), "{lines}");
    assert!(lines.contains("Jev key: FAILED (missing)"), "{lines}");
    assert_eq!(
        report["keys"],
        json!([
            {
                "key": "EMBEDDING_GEMINI_API_KEY",
                "service": "Google Gemini",
                "state": "accepted",
                "detail": "the key was accepted; Google gives no balance to a key, so the credit is on the billing page",
                "billing_page": "https://aistudio.google.com/billing",
            },
            {
                "key": "CONVERTER_JEV_API_KEY",
                "service": "Jev",
                "state": "missing",
                "detail": "CONVERTER_JEV_API_KEY is not set; add it to .env or to the environment, then start the server again",
                "billing_page": "https://console.typesafe.ai/",
            },
        ])
    );
    let stand_in = json!({ "provider": "stand-in", "model": "stand-in", "thinking": "medium" });
    let model = |work: &str, tools: Value| {
        let mut model = stand_in.clone();
        model["work"] = json!(work);
        model["tools"] = tools;
        model
    };
    assert_eq!(
        report["models"],
        json!([
            model("ask", json!(["search", "answer"])),
            model("ingest", json!(["ingest_pdf", "write_insights"])),
            model("datasets", json!(["build_dataset"])),
        ])
    );

    let asked = async |gemini: KeyState, jev: KeyState| {
        let services = StandInServices::new().with_key_states(KeyStates { gemini, jev });
        let client = connect(QuantyServer::new(ports.config.clone(), services)).await;
        let result = call(&client, "health", json!({})).await;
        structured(&result).clone()
    };

    let refused = asked(
        KeyState::Refused,
        KeyState::Unread(KeyReadFailure::TimedOut),
    )
    .await;

    assert_eq!(refused["healthy"], false);
    assert_eq!(refused["keys"][0]["state"], "refused");
    assert_eq!(
        refused["keys"][0]["detail"],
        "the provider refused the key; check it on the billing page, put a good key in .env, then start the server again"
    );
    assert_eq!(refused["keys"][1]["state"], "unread");
    assert_eq!(
        refused["keys"][1]["detail"],
        "the provider gave no answer within 5 seconds"
    );
    let lines = refused["report"].as_str().unwrap();
    assert!(lines.contains("Gemini key: FAILED (refused)"), "{lines}");
    assert!(lines.contains("Jev key: FAILED (unread)"), "{lines}");

    let spent = asked(
        KeyState::OutOfCredit,
        KeyState::Unread(KeyReadFailure::Status(500)),
    )
    .await;

    assert_eq!(spent["healthy"], false);
    assert_eq!(spent["keys"][0]["state"], "out_of_credit");
    assert_eq!(
        spent["keys"][0]["detail"],
        "the provider says the prepaid credit is used up; add credit on the billing page"
    );
    assert_eq!(
        spent["keys"][1]["detail"],
        "the provider answered with HTTP 500"
    );
    let lines = spent["report"].as_str().unwrap();
    assert!(
        lines.contains("Gemini key: FAILED (out_of_credit)"),
        "{lines}"
    );
    assert_eq!(
        spent["keys"][0]["billing_page"],
        "https://aistudio.google.com/billing"
    );
    assert_eq!(
        spent["keys"][1]["billing_page"],
        "https://console.typesafe.ai/"
    );

    let refused_location = asked(KeyState::LocationRefused, KeyState::Accepted).await;

    assert_eq!(refused_location["healthy"], false);
    assert_eq!(refused_location["keys"][0]["state"], "location_refused");
    let detail = refused_location["keys"][0]["detail"].as_str().unwrap();
    assert!(
        detail.contains("EMBEDDING_GEMINI_IPV4_ONLY=true"),
        "{detail}"
    );
    let lines = refused_location["report"].as_str().unwrap();
    assert!(
        lines.contains("Gemini key: FAILED (location_refused)"),
        "{lines}"
    );
}
