//! Rejected artifact replies exercise inventory, shape and display constraints.

use serde_json::json;

type Rejection = (fn(&mut serde_json::Value), &'static str);

pub const REJECTIONS: &[Rejection] = &[
    (|v| v["artifacts"] = json!([]), "missing ids"),
    (
        |v| v["artifacts"][0]["title"] = json!("x".repeat(81)),
        "title: 81 characters exceeds 80",
    ),
    (
        |v| v["artifacts"][0]["description"] = json!("x".repeat(281)),
        "description: 281 characters exceeds 280",
    ),
    (
        |v| v["artifacts"][0]["description"] = json!("A *bold* description"),
        "description: Markdown formatting",
    ),
    (
        |v| {
            let first = v["artifacts"][0].clone();
            v["artifacts"].as_array_mut().unwrap().push(first);
        },
        "duplicate id",
    ),
    (|v| v["artifacts"][0]["id"] = json!("unknown"), "unknown id"),
    (|v| v["artifacts"][0]["title"] = json!(" "), "title: empty"),
    (
        |v| v["artifacts"][0]["title"] = json!(" padded"),
        "title: leading or trailing whitespace",
    ),
    (
        |v| v["artifacts"][0]["title"] = json!("é".repeat(81)),
        "title: 81 characters exceeds 80",
    ),
    (
        |v| v["artifacts"][0]["description"] = json!("界".repeat(281)),
        "description: 281 characters exceeds 280",
    ),
    (
        |v| v["artifacts"][0]["description"] = json!("two\nparagraphs"),
        "description: line or paragraph break at character 4",
    ),
    (
        |v| v["artifacts"][0]["description"] = json!("a\u{2029}b"),
        "description: line or paragraph break at character 2",
    ),
    (
        |v| v["artifacts"][0]["title"] = json!("A **bold** title"),
        "title: Markdown formatting",
    ),
    (
        |v| v["artifacts"][0]["description"] = json!("Uses `x` here"),
        "description: Markdown formatting",
    ),
    (
        |v| v["artifacts"][0]["title"] = json!("# Heading"),
        "title: Markdown formatting",
    ),
    (
        |v| v["artifacts"][0]["description"] = json!("* Emphasis"),
        "description: Markdown formatting",
    ),
    (
        |v| v["artifacts"][0]["title"] = json!("a\u{0}b"),
        "title: control character at character 2",
    ),
    (
        |v| v["artifacts"][0]["title"] = json!("\u{0}".repeat(10_000)),
        "title: 10000 characters exceeds 80",
    ),
    (
        |v| v["artifacts"][0]["extra"] = json!(true),
        "invalid response shape",
    ),
    (|v| v["extra"] = json!(true), "invalid response shape"),
];
