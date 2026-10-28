//! Checks the three tools that lead an agent to the others, and the table of the docs that lists
//! them.

use std::path::Path;

use serde_json::json;

use crate::support::{ClosedPorts, call, connect, connect_on_2026_07_28, error_text, structured};

const GUIDE_TOPICS: [&str; 6] = ["start", "research", "ingest", "jobs", "costs", "errors"];

/// The most bytes of a guide: it is read by a model, so it stays short.
const GUIDE_BUDGET: usize = 6144;

#[tokio::test(flavor = "multi_thread")]
async fn catalog_leads_from_groups_to_tools_and_describe_tool_and_guide_give_the_rest() {
    let ports = ClosedPorts::new();
    let client = connect(ports.server()).await;
    let tools = client.list_all_tools().await.unwrap();
    let catalog_tool = tools.iter().find(|tool| tool.name == "catalog").unwrap();
    assert_eq!(
        serde_json::to_value(&catalog_tool.meta).unwrap()["jet/discovery"],
        json!({ "version": 1, "entryArguments": {} })
    );

    let root = call(&client, "catalog", json!({})).await;
    let root = structured(&root);

    assert_eq!(root["version"], 1);
    let nodes = root["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 8);
    for (node, group) in nodes.iter().zip(root["groups"].as_array().unwrap()) {
        assert_eq!(node["kind"], "discover");
        assert_eq!(node["tool"], "catalog");
        assert_eq!(node["arguments"], json!({ "group": group["name"] }));
        assert!(!group["about"].as_str().unwrap().is_empty());
        let first = &group["tools"][0];
        assert!(!first["summary"].as_str().unwrap().is_empty());
        assert!(first["paid"].is_boolean());
    }
    let start_here = root["start_here"].as_array().unwrap();
    assert!(!start_here.is_empty() && start_here.len() <= 5);
    for step in start_here {
        assert!(step["tool"].is_string() && step["why"].is_string());
    }

    let library = call(&client, "catalog", json!({ "group": "library" })).await;
    let library = structured(&library);

    assert_eq!(library["groups"].as_array().unwrap().len(), 1);
    let leaves = library["nodes"].as_array().unwrap();
    let listed: Vec<&str> = library["groups"][0]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(listed.len(), leaves.len());
    for (leaf, name) in leaves.iter().zip(&listed) {
        assert_eq!(leaf["kind"], "tool");
        assert_eq!(leaf["tool"], *name);
        assert!(leaf["arguments"].is_object());
    }
    let overview = leaves
        .iter()
        .find(|leaf| leaf["tool"] == "document_overview");
    assert!(overview.unwrap()["arguments"]["document_id"].is_string());

    let names = call(&client, "catalog", json!({ "detail": "names" })).await;
    let first_tool = &structured(&names)["groups"][0]["tools"][0];
    assert_eq!(first_tool, &json!({ "name": "catalog" }));

    let described = call(&client, "describe_tool", json!({ "name": "ingest_pdf" })).await;
    let described = structured(&described);
    assert_eq!(described["name"], "ingest_pdf");
    assert_eq!(described["group"], "ingest");
    assert_eq!(described["paid"], true);
    for field in ["title", "description", "arguments", "cost"] {
        assert!(
            !described[field].as_str().unwrap().is_empty(),
            "{field} of ingest_pdf"
        );
    }
    for field in ["examples", "errors", "related"] {
        assert!(
            !described[field].as_array().unwrap().is_empty(),
            "{field} of ingest_pdf"
        );
    }

    let unknown = call(&client, "describe_tool", json!({ "name": "no_such_tool" })).await;
    let text = error_text(&unknown);
    assert!(
        text.contains("no_such_tool") && text.contains("`catalog`"),
        "{text}"
    );

    for topic in GUIDE_TOPICS {
        let guide = call(&client, "guide", json!({ "topic": topic })).await;
        let guide = structured(&guide);
        assert_eq!(guide["topic"], topic);
        let text = guide["text"].as_str().unwrap();
        assert!(!text.trim().is_empty(), "the guide {topic} is empty");
        assert!(
            text.len() <= GUIDE_BUDGET,
            "the guide {topic} has {} bytes",
            text.len()
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_tool_table_of_the_docs_matches_the_catalog() {
    let ports = ClosedPorts::new();
    let client = connect_on_2026_07_28(ports.server()).await;
    let result = call(&client, "catalog", json!({ "detail": "names" })).await;
    let catalogued: Vec<(String, String)> = structured(&result)["groups"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|group| {
            let name = group["name"].as_str().unwrap().to_owned();
            group["tools"]
                .as_array()
                .unwrap()
                .iter()
                .map(move |tool| (tool["name"].as_str().unwrap().to_owned(), name.clone()))
        })
        .collect();

    let docs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/mcp.md");
    let docs = std::fs::read_to_string(&docs).expect("docs/mcp.md should be readable");
    let documented: Vec<(String, String)> = docs
        .lines()
        .skip_while(|line| !line.starts_with("| Tool | Group |"))
        .skip(2)
        .take_while(|line| line.starts_with('|'))
        .map(|row| {
            let cells: Vec<&str> = row.split('|').map(str::trim).collect();
            (cells[1].trim_matches('`').to_owned(), cells[2].to_owned())
        })
        .collect();

    assert_eq!(documented, catalogued);
}
