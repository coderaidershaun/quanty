//! Checks that the tool list is small enough to put before a model, stays in one order, and that
//! the catalog leads to every tool in it.

use std::collections::BTreeSet;

use serde_json::{Value, json};

use crate::support::{ClosedPorts, call, connect_on_2026_07_28, structured};

/// What the model reads of the list: the name, title, description and input schema of each tool.
const MODEL_FACING_BUDGET: usize = 24 * 1024;

/// The whole list as it travels. There are two caps because the model reads only the name, title,
/// description and input schema; the output schemas and `_meta` stay with the host.
const WHOLE_LIST_BUDGET: usize = 48 * 1024;

/// The size of the list of 18 tools at 148dc9d, before the research tools were added.
const BYTES_BEFORE: usize = 70_132;

fn compact_len(value: &impl serde::Serialize) -> usize {
    serde_json::to_string(value).unwrap().len()
}

fn names_of(tools: &[rmcp::model::Tool]) -> Vec<String> {
    tools.iter().map(|tool| tool.name.to_string()).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_tool_list_fits_its_budget_in_a_fixed_order_and_every_tool_is_catalogued() {
    let ports = ClosedPorts::new();
    // The rmcp client keeps a list that has a time to live, so each read needs its own connection.
    let first = connect_on_2026_07_28(ports.server()).await;
    let listed = first.list_tools(None).await.unwrap();
    let second = connect_on_2026_07_28(ports.server()).await;
    let listed_again = second.list_tools(None).await.unwrap();
    let tools = &listed.tools;

    let whole = compact_len(&listed);
    let model_facing: usize = tools
        .iter()
        .map(|tool| {
            1 + compact_len(&json!({
                "name": tool.name,
                "title": tool.title,
                "description": tool.description,
                "inputSchema": tool.input_schema,
            }))
        })
        .sum();
    println!(
        "{} tools: {model_facing} bytes for the model (cap {MODEL_FACING_BUDGET}), {whole} bytes whole (cap {WHOLE_LIST_BUDGET}), {BYTES_BEFORE} bytes before",
        tools.len()
    );
    assert!(model_facing <= MODEL_FACING_BUDGET, "{model_facing} bytes");
    assert!(whole <= WHOLE_LIST_BUDGET, "{whole} bytes");
    assert!(whole < BYTES_BEFORE, "{whole} bytes");

    assert_eq!(names_of(tools), names_of(&listed_again.tools));

    for tool in tools {
        let name = &tool.name;
        let schema = tool.output_schema.as_ref();
        let schema = schema.unwrap_or_else(|| panic!("{name} declares its output schema"));
        assert_eq!(schema.get("type"), Some(&json!("object")), "{name}");
        assert!(
            tool.title.as_deref().is_some_and(|title| !title.is_empty()),
            "{name} has a title"
        );
        let marks = serde_json::to_value(&tool.meta).unwrap();
        assert!(marks["quanty/group"].is_string(), "{name} has a group");
        let paid = marks["quanty/cost"]["paid"]
            .as_bool()
            .unwrap_or_else(|| panic!("{name} has a cost"));
        let description = tool.description.as_deref().unwrap();
        let first_sentence = description.split(". ").next().unwrap();
        assert_eq!(first_sentence.contains("PAID"), paid, "{description}");
        assert_eq!(description.contains("PAID"), paid, "{description}");
    }

    let listed_names = names_of(tools);
    let catalog_of = async |arguments: Value| {
        let result = call(&first, "catalog", arguments).await;
        structured(&result).clone()
    };
    let named = |catalog: &Value| -> Vec<String> {
        let groups = catalog["groups"].as_array().unwrap();
        let tools = groups
            .iter()
            .flat_map(|group| group["tools"].as_array().unwrap());
        tools
            .map(|tool| tool["name"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(
        named(&catalog_of(json!({ "detail": "names" })).await),
        listed_names
    );
    let groups = catalog_of(json!({})).await["groups"].clone();
    for group in groups.as_array().unwrap() {
        let name = group["name"].as_str().unwrap();
        let alone = catalog_of(json!({ "group": name })).await;
        let in_list: Vec<String> = tools
            .iter()
            .filter(|tool| serde_json::to_value(&tool.meta).unwrap()["quanty/group"] == name)
            .map(|tool| tool.name.to_string())
            .collect();
        assert_eq!(named(&alone), in_list, "the tools of {name}");
    }

    let listed_set: BTreeSet<&str> = listed_names.iter().map(String::as_str).collect();
    for tool in tools {
        let result = call(&first, "describe_tool", json!({ "name": tool.name })).await;
        let described = structured(&result);
        let properties = tool.input_schema["properties"].as_object().unwrap();
        for example in described["examples"].as_array().unwrap() {
            let example = example
                .as_object()
                .unwrap_or_else(|| panic!("an example of {} is an object", tool.name));
            for key in example.keys() {
                assert!(
                    properties.contains_key(key),
                    "an example of {} has `{key}`, which is no argument",
                    tool.name
                );
            }
        }
        for related in described["related"].as_array().unwrap() {
            let related = related.as_str().unwrap();
            assert!(
                listed_set.contains(related),
                "{} is related to {related}, which is no tool",
                tool.name
            );
        }
    }
}
