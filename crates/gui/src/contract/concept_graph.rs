//! The concepts and results of one answer, and how they link, ready to draw as a graph.

use super::ids::ConceptId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NodeId {
    Concept(ConceptId),
    /// The number of a result.
    Item(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NodeKind {
    Concept,
    Related,
    Formula,
    Figure,
    Table,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GraphNode {
    pub id: NodeId,
    pub kind: NodeKind,
    /// A concept's name, or a result's label such as "Formula (2.4)".
    pub label: String,
    /// A concept's definition, or a result's name or caption.
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EdgeKind {
    DerivedFrom,
    Assumes,
    Generalises,
    PartOf,
    UsedFor,
    Mentions,
}

/// Between two concepts the direction is the one stored; a result points at the concepts it
/// mentions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GraphEdge {
    pub from: NodeId,
    pub to: NodeId,
    pub kind: EdgeKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ConceptGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}
