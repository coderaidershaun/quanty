//! The parts of a result that several concept tools share: a concept in brief, a relation with the
//! place of the item that stated it, an item that mentions a concept, and a node for a host that
//! explores tools by calling them.

use std::collections::HashMap;

use graph::{ConceptItem, ConceptNode, ItemPlace, Relation};
use rag_core::{ConceptId, ItemId};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Serialize, JsonSchema)]
pub(super) struct ConceptBrief {
    pub(super) concept_id: String,
    pub(super) name: String,
    pub(super) definition: String,
}

impl From<&ConceptNode> for ConceptBrief {
    fn from(concept: &ConceptNode) -> ConceptBrief {
        ConceptBrief {
            concept_id: concept.id.to_string(),
            name: concept.name.clone(),
            definition: concept.definition.clone(),
        }
    }
}

#[derive(Serialize, JsonSchema)]
pub(super) struct StatedBy {
    item_id: String,
    document_id: String,
    page: u32,
}

/// A relation between two concepts, from the one it leaves to the one it reaches.
#[derive(Serialize, JsonSchema)]
pub(super) struct EdgeView {
    from: String,
    to: String,
    kind: String,
    /// `null` when the item that stated it is no longer stored.
    stated_by: Option<StatedBy>,
}

impl EdgeView {
    pub(super) fn new(relation: &Relation, places: &HashMap<ItemId, ItemPlace>) -> EdgeView {
        EdgeView {
            from: relation.from.to_string(),
            to: relation.to.to_string(),
            kind: relation.kind.as_str().to_owned(),
            stated_by: places.get(&relation.item).map(|place| StatedBy {
                item_id: relation.item.to_string(),
                document_id: place.document.to_string(),
                page: place.page,
            }),
        }
    }
}

/// An item that mentions a concept, with the place to read it.
#[derive(Serialize, JsonSchema)]
pub(super) struct ItemRow {
    item_id: String,
    document_id: String,
    page: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    printed_page: Option<String>,
    kind: String,
    /// The name the item gave the concept.
    wording: String,
}

impl From<ConceptItem> for ItemRow {
    fn from(item: ConceptItem) -> ItemRow {
        ItemRow {
            item_id: item.item.to_string(),
            document_id: item.document.to_string(),
            page: item.page,
            printed_page: item.printed_page,
            kind: item.kind.as_str().to_owned(),
            wording: item.wording,
        }
    }
}

/// The version that the nodes of a result carry, which a host that explores tools reads.
pub(super) const NODE_VERSION: u32 = 1;

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum NodeKind {
    Discover,
    Tool,
}

/// A call that a host which explores tools shows, and makes when a person opens it.
#[derive(Serialize, JsonSchema)]
pub(super) struct Node {
    label: String,
    kind: NodeKind,
    tool: &'static str,
    arguments: Map<String, Value>,
}

impl Node {
    pub(super) fn tool(label: &str, tool: &'static str, concept: ConceptId) -> Node {
        Node::new(label, NodeKind::Tool, tool, concept)
    }

    /// The node that opens a concept.
    pub(super) fn concept(concept: &ConceptNode) -> Node {
        Node::new(&concept.name, NodeKind::Discover, "concept", concept.id)
    }

    fn new(label: &str, kind: NodeKind, tool: &'static str, concept: ConceptId) -> Node {
        let arguments =
            Map::from_iter([("concept_id".to_owned(), Value::String(concept.to_string()))]);
        Node {
            label: label.to_owned(),
            kind,
            tool,
            arguments,
        }
    }
}
