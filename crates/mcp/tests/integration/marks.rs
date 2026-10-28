//! Checks of the `_meta` marks and the result schemas that the tool list carries for the tools
//! that explore concepts, which a host reads to group the tools and to price them.

use rmcp::model::Tool;
use serde_json::json;

/// The marks of the tools that explore concepts: the group, the cost, the entry point of the
/// explorer on `concept` only, a title and the schema of the result. Only `find_concepts` can be
/// paid, and its first sentence says so.
pub fn assert_concept_tools_are_marked_and_have_schemas(tools: &[Tool]) {
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
