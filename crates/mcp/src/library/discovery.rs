//! The nodes that a host draws from a result, so that a person can browse the library from media
//! to documents to sections without asking a model.

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

/// The version of the node list that a host reads.
const VERSION: u32 = 1;

/// The nodes of a result that a host can open: each names the tool to call and its arguments.
#[derive(Serialize, JsonSchema)]
pub(crate) struct Discovery {
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

#[derive(Serialize, JsonSchema, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum NodeKind {
    /// A tool that lists more nodes when it is called with these arguments.
    Discover,
    /// A tool that reads something and lists no node.
    Tool,
}

impl Discovery {
    pub(crate) fn new() -> Discovery {
        Discovery {
            version: VERSION,
            nodes: Vec::new(),
        }
    }

    /// A node for a tool that lists nodes in its turn.
    pub(crate) fn discovering(
        mut self,
        label: &str,
        tool: &'static str,
        arguments: Value,
    ) -> Discovery {
        self.nodes.push(Node {
            label: label.to_owned(),
            kind: NodeKind::Discover,
            tool,
            arguments,
        });
        self
    }

    /// A node for a tool that gives what it reads and no more nodes.
    pub(crate) fn reading(
        mut self,
        label: &str,
        tool: &'static str,
        arguments: Value,
    ) -> Discovery {
        self.nodes.push(Node {
            label: label.to_owned(),
            kind: NodeKind::Tool,
            tool,
            arguments,
        });
        self
    }
}
