//! Checks the tools that explore the concept graph, over a library whose concepts, mentions and
//! relations were scripted, so that every node and edge is known.

use serde_json::{Value, json};

use crate::support::concept_graph::{BLACK_SCHOLES, ITO, STOCHASTIC_CALCULUS, VOLATILITY};
use crate::support::{Client, Library, call, connect, sample_notes_document_id, structured};

mod documents;
mod finding;
mod walking;

const NOTES_1_TITLE: &str = "Quanty Sample Notes, chapter 1: Options Pricing Intuition";
const NOTES_2_TITLE: &str = "Quanty Sample Notes, chapter 2: Black Scholes In Depth";

/// The ids that results gave for the scripted concepts.
struct Ids {
    black_scholes: String,
    volatility: String,
    ito: String,
    stochastic: String,
}

impl Ids {
    async fn read(client: &Client) -> Ids {
        let id_of = async |name: &str| {
            let found = call(client, "find_concepts", json!({ "name": name })).await;
            let id = structured(&found)["concepts"][0]["concept_id"].clone();
            id.as_str()
                .unwrap_or_else(|| panic!("{name} should be stored"))
                .to_owned()
        };
        Ids {
            black_scholes: id_of(BLACK_SCHOLES).await,
            volatility: id_of(VOLATILITY).await,
            ito: id_of(ITO).await,
            stochastic: id_of(STOCHASTIC_CALCULUS).await,
        }
    }
}

fn notes(chapter: u32) -> String {
    sample_notes_document_id(chapter).to_string()
}

fn texts<'a>(values: &'a Value, key: &str) -> Vec<&'a str> {
    values
        .as_array()
        .expect("the value is a list")
        .iter()
        .map(|value| value[key].as_str().expect("the field is text"))
        .collect()
}

fn kinds(edges: &Value) -> Vec<&str> {
    let mut kinds = texts(edges, "kind");
    kinds.sort();
    kinds
}

fn edge_of_kind<'a>(edges: &'a Value, kind: &str) -> &'a Value {
    edges
        .as_array()
        .expect("the edges are a list")
        .iter()
        .find(|edge| edge["kind"] == kind)
        .unwrap_or_else(|| panic!("no edge of kind {kind} in {edges}"))
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p mcp --test integration -- --ignored concepts::"]
async fn the_concept_graph_is_explored_through_every_concept_tool() {
    let library = Library::ingested_with_concept_graph("mcp-concepts").await;
    let client = connect(library.server()).await;
    let ids = Ids::read(&client).await;

    finding::find_concepts_finds_by_name_alias_and_meaning(&library, &client).await;
    finding::concept_gives_a_card_and_the_nodes_to_explore_from(&client, &ids).await;
    walking::concept_neighbours_follow_kind_direction_and_depth(&client, &ids).await;
    walking::concept_items_come_a_page_at_a_time(&client, &ids).await;
    walking::concept_path_crosses_edges_either_way_and_is_empty_when_there_is_none(&client, &ids)
        .await;
    documents::compare_documents_splits_shared_from_only_in(&client).await;
    documents::similar_items_embeds_nothing(&library).await;
    documents::document_concepts_adds_ids_limits_and_counts(&client).await;
}
