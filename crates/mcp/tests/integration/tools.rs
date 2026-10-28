//! Checks what an agent sees before it calls anything: the list of tools with their input
//! schemas, and the report of `health`.

use rmcp::model::{CacheScope, ProtocolVersion, Tool};
use serde_json::{Value, json};

use mcp::QuantyServer;
use rag_ingestion::{KeyReadFailure, KeyState, KeyStates};

use crate::support::{
    ClosedPorts, StandInServices, call, connect, connect_on_2026_07_28, structured,
};

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

/// The marks a host reads to group the job tools, to price them and to draw a progress bar from
/// them, and the schema of what each gives back.
fn assert_job_tools_are_marked_and_have_schemas(tools: &[Tool]) {
    let tool = |name: &str| tools.iter().find(|tool| tool.name == name).unwrap();
    let free = json!({ "paid": false });
    let paid = |who: &str| json!({ "paid": true, "who": who });
    let job_mark = json!({ "version": 1, "statusTool": "job_status", "idField": "job_id" });
    let marked = [
        ("check_pdf", "ingest", free.clone(), false),
        ("ingest_pdf", "ingest", paid("ingest"), true),
        ("write_insights", "ingest", paid("ingest"), true),
        ("build_dataset", "datasets", paid("datasets"), true),
        ("job_status", "jobs", free.clone(), true),
        ("watch_job", "jobs", free.clone(), true),
        ("list_jobs", "jobs", free.clone(), false),
        ("stop_job", "jobs", free.clone(), false),
        ("ingest_status", "jobs", free.clone(), false),
        ("insights_status", "jobs", free.clone(), false),
        ("dataset_status", "jobs", free, false),
    ];
    for (name, group, cost, starts_a_job) in marked {
        let marks = serde_json::to_value(&tool(name).meta).unwrap();
        assert_eq!(marks["quanty/group"], group, "the group of {name}");
        assert_eq!(marks["quanty/cost"], cost, "the cost of {name}");
        assert_eq!(
            marks.get("jet/job"),
            starts_a_job.then_some(&job_mark),
            "{name}"
        );
    }
    let output_keys = |name: &str| -> Vec<String> {
        let schema = tool(name).output_schema.as_ref();
        let schema = schema.unwrap_or_else(|| panic!("{name} declares its output schema"));
        assert_eq!(schema.get("type"), Some(&json!("object")), "{name}");
        let properties = schema["properties"].as_object().unwrap();
        properties.keys().cloned().collect()
    };
    let job_keys = [
        "job_id",
        "kind",
        "state",
        "stage",
        "started_at",
        "updated_at",
    ];
    for name in [
        "job_status",
        "watch_job",
        "ingest_pdf",
        "write_insights",
        "build_dataset",
        "ingest_status",
        "insights_status",
        "dataset_status",
    ] {
        let keys = output_keys(name);
        for key in job_keys.into_iter().chain(["usage", "cost_usd", "error"]) {
            assert!(keys.iter().any(|k| k == key), "{name} has {key}: {keys:?}");
        }
    }
    let check = tool("check_pdf");
    assert!(check.title.is_some(), "check_pdf has a title");
    let check_keys = output_keys("check_pdf");
    for key in ["title", "pages", "state", "blockers", "estimate"] {
        assert!(check_keys.iter().any(|k| k == key), "check_pdf has {key}");
    }
    let check_description = check
        .description
        .as_deref()
        .expect("check_pdf has a description");
    assert!(
        check_description.len() <= 300 && check_description.contains("Free"),
        "{check_description}"
    );
    let list_keys = output_keys("list_jobs");
    assert!(list_keys.contains(&"running".to_owned()) && list_keys.contains(&"jobs".to_owned()));
    assert!(output_keys("stop_job").contains(&"state".to_owned()));
}

/// The tools that browse and read the library: each has a title, a short description, the schema
/// of its result and a group, and only three of them are marked for a host that discovers tools.
fn assert_library_tools_are_marked_and_have_schemas(tools: &[Tool]) {
    let tool = |name: &str| tools.iter().find(|tool| tool.name == name).unwrap();
    let free = json!({ "paid": false });
    let new_tools = [
        ("list_media", "library"),
        ("document_overview", "library"),
        ("document_outline", "library"),
        ("read_section", "library"),
        ("read_pages", "library"),
        ("read_item", "library"),
        ("read_many", "research"),
    ];
    for (name, group) in new_tools {
        let tool = tool(name);
        let marks = serde_json::to_value(&tool.meta).unwrap();
        assert_eq!(marks["quanty/group"], group, "the group of {name}");
        assert_eq!(marks["quanty/cost"], free, "the cost of {name}");
        assert!(tool.title.is_some(), "{name} has a title");
        let description = tool.description.as_deref().unwrap();
        assert!(
            description.len() < 300 && !description.contains("PAID"),
            "{name} has a short description of a free tool: {description}"
        );
        let schema = tool.output_schema.as_ref().unwrap();
        assert_eq!(schema.get("type"), Some(&json!("object")), "{name}");
    }
    let entry = json!({ "version": 1, "entryArguments": { "detail": "names" } });
    let target = json!({ "version": 1 });
    let discovery = |name: &str| {
        serde_json::to_value(&tool(name).meta)
            .unwrap()
            .get("jet/discovery")
            .cloned()
    };
    assert_eq!(discovery("list_media"), Some(entry.clone()));
    assert_eq!(discovery("list_documents"), Some(entry));
    assert_eq!(discovery("document_outline"), Some(target));
    for name in [
        "document_overview",
        "read_section",
        "read_pages",
        "read_item",
        "read_many",
        "read_page",
        "search",
    ] {
        assert_eq!(discovery(name), None, "{name} is not a discovery tool");
    }
    let documents = tool("list_documents");
    assert_eq!(documents.title.as_deref(), Some("List documents"));
    assert!(documents.output_schema.is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn the_server_lists_every_tool_with_its_input_schema() {
    let ports = ClosedPorts::new();
    let client = connect(ports.server()).await;

    let tools = client.list_all_tools().await.unwrap();

    let names: Vec<&str> = tools.iter().map(|tool| tool.name.as_ref()).collect();
    let expected_names = [
        "answer",
        "build_dataset",
        "chapter_concepts",
        "chapter_contents",
        "chapter_insights",
        "check_pdf",
        "dataset_status",
        "document_outline",
        "document_overview",
        "edit_document_tags",
        "edit_media",
        "export_insights",
        "health",
        "ingest_pdf",
        "ingest_status",
        "insights_status",
        "job_status",
        "list_documents",
        "list_jobs",
        "list_media",
        "read_item",
        "read_many",
        "read_page",
        "read_pages",
        "read_section",
        "search",
        "stop_job",
        "watch_job",
        "write_insights",
    ];
    assert_eq!(names, expected_names);
    let expected: [(&str, &[&str], &[&str]); 29] = [
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
            &["cursor", "document_id", "max_chars", "page"],
            &["document_id"],
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
        ("dataset_status", &["job_id"], &["job_id"]),
        ("document_outline", &["document_id"], &["document_id"]),
        (
            "document_overview",
            &["detail", "document_id"],
            &["document_id"],
        ),
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
                "pdf_url",
                "tags",
            ],
            &["media"],
        ),
        ("ingest_status", &["job_id"], &["job_id"]),
        ("insights_status", &["job_id"], &["job_id"]),
        ("job_status", &["job_id"], &["job_id"]),
        (
            "list_documents",
            &[
                "author", "category", "cursor", "detail", "limit", "media", "tags",
            ],
            &[],
        ),
        ("list_jobs", &[], &[]),
        ("list_media", &["author", "category", "detail", "tag"], &[]),
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
        ("read_many", &["max_chars", "requests"], &["requests"]),
        (
            "read_page",
            &["cursor", "document_id", "max_chars", "page"],
            &["document_id", "page"],
        ),
        (
            "read_pages",
            &["as", "cursor", "document_id", "from", "max_chars", "to"],
            &["document_id", "from", "to"],
        ),
        (
            "read_section",
            &["as", "cursor", "document_id", "max_chars", "section"],
            &["document_id", "section"],
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
        ("stop_job", &["job_id"], &["job_id"]),
        ("watch_job", &["job_id", "timeout_s"], &["job_id"]),
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

// SMELL: this file is over 500 lines because the table of tools grows with each tool. The check
// of `health` below is a subject of its own and can move to a file of its own.
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
