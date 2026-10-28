//! The catalogue of the fixture: the groups of its tools, and the tools of each group with
//! arguments to call them with, in the shape that a host's tool browser reads.

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Value, json};

use super::fake::FakeJob;
use super::{FixtureError, TITLE};

/// The version of the shape that a host's tool browser reads.
const VERSION: u32 = 1;

/// The groups of the fixture's tools, with the name a person reads for each.
const GROUPS: [(&str, &str); 3] = [
    ("library", "Library"),
    ("ingest", "Ingest"),
    ("jobs", "Jobs"),
];

#[derive(Serialize, JsonSchema)]
pub(super) struct Catalog {
    version: u32,
    nodes: Vec<Node>,
}

#[derive(Serialize, JsonSchema)]
struct Node {
    label: String,
    kind: NodeKind,
    tool: &'static str,
    arguments: Value,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum NodeKind {
    /// Call `tool` with `arguments` to see more nodes.
    Discover,
    /// A tool to call with `arguments`.
    Tool,
}

impl Node {
    fn discover(label: &str, group: &str) -> Node {
        Node {
            label: label.to_owned(),
            kind: NodeKind::Discover,
            tool: "catalog",
            arguments: json!({ "group": group }),
        }
    }

    fn tool(label: &str, tool: &'static str, arguments: Value) -> Node {
        Node {
            label: label.to_owned(),
            kind: NodeKind::Tool,
            tool,
            arguments,
        }
    }
}

impl Catalog {
    /// The groups when none is asked for, and the tools of the group when it is. `jobs` are the
    /// fake jobs started so far.
    ///
    /// # Errors
    /// [`FixtureError::UnknownGroup`] when the group is not one of the fixture's.
    pub(super) fn of(group: Option<&str>, jobs: &[FakeJob]) -> Result<Catalog, FixtureError> {
        let nodes = match group {
            None => GROUPS
                .iter()
                .map(|(group, label)| Node::discover(label, group))
                .collect(),
            Some("library") => vec![
                Node::tool(
                    "Search the sample",
                    "search",
                    json!({ "question": "Show the sample figure" }),
                ),
                Node::tool("Read the sample page", "fixture_page", json!({})),
            ],
            Some("ingest") => vec![Node::tool(
                "Ingest a sample PDF",
                "ingest_pdf",
                json!({ "media": TITLE, "file_name": "chapter-1-sample.pdf" }),
            )],
            Some("jobs") => jobs
                .iter()
                .map(|job| {
                    Node::tool(
                        &format!("Watch {}", job.file_name()),
                        "watch_job",
                        json!({ "job_id": job.job_id() }),
                    )
                })
                .collect(),
            Some(other) => {
                return Err(FixtureError::UnknownGroup {
                    group: other.to_owned(),
                });
            }
        };
        Ok(Catalog {
            version: VERSION,
            nodes,
        })
    }
}
