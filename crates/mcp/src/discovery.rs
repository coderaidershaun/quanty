//! The nodes that a host draws from a result, so that a person can browse the library, the
//! concepts and the tools without asking a model.

use rmcp::model::JsonObject;
use schemars::JsonSchema;
use serde::Serialize;

/// The version of the node list that a host reads.
const VERSION: u32 = 1;

/// The nodes of a result that a host can open: each names the tool to call and its arguments.
#[derive(Serialize, JsonSchema)]
pub(crate) struct Discovery {
    version: u32,
    nodes: Vec<Node>,
}

#[derive(Serialize, JsonSchema)]
pub(crate) struct Node {
    label: String,
    kind: NodeKind,
    tool: String,
    arguments: JsonObject,
}

#[derive(Serialize, JsonSchema, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum NodeKind {
    /// A tool that lists more nodes when it is called with these arguments.
    Discover,
    /// A tool that reads something and lists no node.
    Tool,
}

impl Node {
    /// A node for a tool that lists nodes in its turn.
    pub(crate) fn discovering(label: &str, tool: &str, arguments: JsonObject) -> Node {
        Node::new(label, NodeKind::Discover, tool, arguments)
    }

    /// A node for a tool that gives what it reads and no more nodes.
    pub(crate) fn reading(label: &str, tool: &str, arguments: JsonObject) -> Node {
        Node::new(label, NodeKind::Tool, tool, arguments)
    }

    fn new(label: &str, kind: NodeKind, tool: &str, arguments: JsonObject) -> Node {
        Node {
            label: label.to_owned(),
            kind,
            tool: tool.to_owned(),
            arguments,
        }
    }
}

impl Discovery {
    pub(crate) fn new() -> Discovery {
        Discovery::of(Vec::new())
    }

    pub(crate) fn of(nodes: Vec<Node>) -> Discovery {
        Discovery {
            version: VERSION,
            nodes,
        }
    }

    pub(crate) fn with(mut self, node: Node) -> Discovery {
        self.nodes.push(node);
        self
    }
}
