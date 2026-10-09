//! What every test needs: a throwaway graph to work in, and the nodes and edges it writes.

use graph::testing::ThrowawayGraph;
use graph::{ConceptNode, FalkorGraph, ItemNode, Mention, Relation, RelationKind};
use rag_core::{ConceptId, Config, DocId, ItemId, ItemKind, Tag};

const THROWAWAY_PREFIX: &str = "test-graph-";

/// A graph that no one else uses, and the settings that name it. The graph is removed when the
/// first value is dropped, so keep it for the whole test.
pub fn throwaway_config(test_name: &str) -> (ThrowawayGraph, Config) {
    let settings = Config::load().expect("the settings should load");
    let throwaway = ThrowawayGraph::new(&settings, test_name);
    let config = Config {
        falkordb_graph: throwaway.name().to_owned(),
        ..settings
    };
    // The name is copied into the config by hand. Without this check, a copy that is missing would
    // leave the real graph in the config, and the test would write to it.
    assert!(config.falkordb_graph.starts_with(THROWAWAY_PREFIX));
    (throwaway, config)
}

/// A graph that no one else uses, and a connection to it. The graph is removed when the first
/// value is dropped, so keep it for the whole test.
pub async fn throwaway(test_name: &str) -> (ThrowawayGraph, FalkorGraph) {
    let (throwaway, config) = throwaway_config(test_name);
    let graph = FalkorGraph::connect(&config)
        .await
        .expect("FalkorDB should answer");
    (throwaway, graph)
}

pub fn tag(text: &str) -> Tag {
    text.parse().expect("a tag is not empty")
}

pub fn concept(name: &str) -> ConceptNode {
    ConceptNode {
        id: ConceptId::random(),
        name: name.to_owned(),
        normalised_name: name.to_lowercase(),
        definition: format!("{name} in one line"),
    }
}

pub fn item(document: DocId, kind: ItemKind, position: u32, page: u32) -> ItemNode {
    ItemNode {
        id: ItemId::new(document, kind, position),
        kind,
        page,
        printed_page: None,
    }
}

pub fn mention(item: &ItemNode, concept: &ConceptNode, wording: &str) -> Mention {
    Mention {
        item: item.id,
        concept: concept.id,
        wording: wording.to_owned(),
    }
}

pub fn relation(
    from: &ConceptNode,
    kind: RelationKind,
    to: &ConceptNode,
    item: &ItemNode,
) -> Relation {
    Relation {
        from: from.id,
        to: to.id,
        kind,
        item: item.id,
    }
}
