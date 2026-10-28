//! Checks of the `_meta` marks, the annotations and the result schemas that the tool list carries,
//! which a host reads to group the tools, to price them and to draw a progress bar.

use rmcp::model::Tool;
use serde_json::json;

/// The marks a host reads to group the job tools, to price them and to draw a progress bar from
/// them, and the schema of what each gives back.
pub(super) fn assert_job_tools_are_marked_and_have_schemas(tools: &[Tool]) {
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
pub(super) fn assert_library_tools_are_marked_and_have_schemas(tools: &[Tool]) {
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

/// The marks of the tools that explore concepts: the group, the cost, the entry point of the
/// explorer on `concept` only, a title and the schema of the result. Only `find_concepts` can be
/// paid, and its first sentence says so.
pub(super) fn assert_concept_tools_are_marked_and_have_schemas(tools: &[Tool]) {
    let tool = |name: &str| tools.iter().find(|tool| tool.name == name).unwrap();
    let names = [
        "find_concepts",
        "concept",
        "concept_neighbours",
        "concept_items",
        "concept_path",
        "compare_documents",
        "document_concepts",
        "similar_items",
    ];
    for name in names {
        let tool = tool(name);
        let marks = serde_json::to_value(&tool.meta).unwrap();
        let cost = if name == "find_concepts" {
            json!({ "paid": true, "who": "gemini" })
        } else {
            json!({ "paid": false })
        };
        assert_eq!(marks["quanty/group"], "concepts", "the group of {name}");
        assert_eq!(marks["quanty/cost"], cost, "the cost of {name}");
        let discovery = (name == "concept").then(|| json!({ "version": 1, "entryArguments": {} }));
        assert_eq!(marks.get("jet/discovery"), discovery.as_ref(), "{name}");
        assert_eq!(marks.get("jet/job"), None, "{name}");
        assert!(
            tool.title.as_deref().is_some_and(|title| !title.is_empty()),
            "{name} has a title"
        );
        let schema = tool.output_schema.as_ref();
        let schema = schema.unwrap_or_else(|| panic!("{name} declares its output schema"));
        assert_eq!(schema.get("type"), Some(&json!("object")), "{name}");
        assert_eq!(
            tool.annotations.as_ref().and_then(|a| a.read_only_hint),
            Some(true),
            "{name} only reads"
        );
    }
    let find = tool("find_concepts").description.as_deref().unwrap();
    let first_sentence = find.split(". ").next().unwrap();
    assert!(first_sentence.contains("PAID"), "{find}");
    assert!(find.contains("`name` is free"), "{find}");
    for name in names.into_iter().filter(|name| *name != "find_concepts") {
        let description = tool(name).description.as_deref().unwrap();
        assert!(!description.contains("PAID"), "{description}");
    }
}
