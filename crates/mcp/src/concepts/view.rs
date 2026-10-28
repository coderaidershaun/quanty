//! The parts of a result that several concept tools share: a concept in brief, a relation with the
//! place of the item that stated it, an item that mentions a concept, and the nodes for a host
//! that explores tools by calling them.

use std::collections::HashMap;

use graph::{ConceptItem, ConceptNode, ItemPlace, Relation};
use rag_core::{ConceptId, ItemId};
use rmcp::model::JsonObject;
use schemars::JsonSchema;
use serde::Serialize;

use crate::discovery::Node;

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

/// The node that opens a tool for one concept, by its id.
pub(super) fn tool_node(label: &str, tool: &str, concept: ConceptId) -> Node {
    Node::reading(label, tool, concept_arguments(concept))
}

/// The node that opens a concept.
pub(super) fn concept_node(concept: &ConceptNode) -> Node {
    Node::discovering(&concept.name, "concept", concept_arguments(concept.id))
}

fn concept_arguments(concept: ConceptId) -> JsonObject {
    JsonObject::from_iter([("concept_id".to_owned(), concept.to_string().into())])
}
