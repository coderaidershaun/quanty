//! `find_concepts` by name, alias and meaning, and the card and nodes of `concept`.

use super::{Ids, NOTES_1_TITLE, NOTES_2_TITLE, notes, texts};
use crate::support::concept_graph::{
    ALIAS, BLACK_SCHOLES, BLACK_SCHOLES_DEFINITION, ITO, VOLATILITY, VOLATILITY_QUERY,
    volatility_vector,
};
use crate::support::{
    Client, Library, StandInServices, call, connect, error_text, sample_content_folder, structured,
};
use rag_core::ConceptId;
use rag_ingestion::testing::StandInEmbedder;
use serde_json::json;

pub(super) async fn find_concepts_finds_by_name_alias_and_meaning(
    library: &Library,
    client: &Client,
) {
    let by_name = call(
        client,
        "find_concepts",
        json!({ "name": "  BLACK-SCHOLES Model " }),
    )
    .await;

    let found = structured(&by_name);
    assert!(found.get("usage").is_none(), "a name costs nothing");
    let concept = &found["concepts"][0];
    assert_eq!(found["concepts"].as_array().unwrap().len(), 1);
    assert_eq!(concept["name"], BLACK_SCHOLES);
    assert_eq!(concept["definition"], BLACK_SCHOLES_DEFINITION);
    assert_eq!(concept["aliases"], json!([ALIAS]));
    assert_eq!(concept["mentions"], 5);
    assert_eq!(concept["documents"], 2);
    assert!(concept.get("score").is_none());

    // The alias, and the name written another way, find the same concept.
    let by_alias = call(
        client,
        "find_concepts",
        json!({ "name": "black scholes merton model" }),
    )
    .await;
    assert_eq!(
        structured(&by_alias)["concepts"][0]["concept_id"],
        concept["concept_id"]
    );
    let nothing = call(
        client,
        "find_concepts",
        json!({ "name": "no such concept" }),
    )
    .await;
    assert_eq!(structured(&nothing)["concepts"], json!([]));

    // The query is placed at the vector of Volatility, so it is nearest to it, and the Black–
    // Scholes model, which is placed 0.25 from it, comes second.
    let embedder = || StandInEmbedder::default().placing(VOLATILITY_QUERY, volatility_vector());
    let services = StandInServices::new().with_embedder(embedder);
    let asking = connect(library.server_over(&sample_content_folder(), services)).await;
    let by_meaning = call(
        &asking,
        "find_concepts",
        json!({ "query": VOLATILITY_QUERY, "limit": 2 }),
    )
    .await;
    let found = structured(&by_meaning);
    assert_eq!(
        texts(&found["concepts"], "name"),
        [VOLATILITY, BLACK_SCHOLES]
    );
    let score = found["concepts"][0]["score"].as_f64().unwrap();
    assert!(score > 0.99, "{score}");
    assert!(!found["usage"].as_array().unwrap().is_empty());
    assert!(found.get("cost_usd").is_some());

    for arguments in [
        json!({ "name": "  ", "query": "" }),
        json!({ "name": BLACK_SCHOLES, "query": VOLATILITY_QUERY }),
    ] {
        let both = call(client, "find_concepts", arguments).await;
        let text = error_text(&both);
        assert!(text.contains("exactly one of `name` and `query`"), "{text}");
    }
}

pub(super) async fn concept_gives_a_card_and_the_nodes_to_explore_from(client: &Client, ids: &Ids) {
    let by_name = call(client, "concept", json!({ "name": "black-scholes model" })).await;
    let by_id = call(
        client,
        "concept",
        json!({ "concept_id": ids.black_scholes }),
    )
    .await;

    let card = structured(&by_id);
    assert_eq!(structured(&by_name), card);
    assert_eq!(card["version"], 1);
    assert_eq!(card["concept_id"], ids.black_scholes.as_str());
    assert_eq!(card["name"], BLACK_SCHOLES);
    assert_eq!(card["definition"], BLACK_SCHOLES_DEFINITION);
    assert_eq!(card["aliases"], json!([ALIAS]));
    assert_eq!(card["mentions"], 5);
    assert_eq!(
        card["documents"],
        json!([
            { "document_id": notes(2), "title": NOTES_2_TITLE, "mentions": 3 },
            { "document_id": notes(1), "title": NOTES_1_TITLE, "mentions": 2 },
        ])
    );
    // The items come by document and page, and each carries the name it gave the concept. The
    // formula on page 3 of the second chapter is asked about with the sentence before it, which
    // names the Black–Scholes model.
    let rows = card["top_items"].as_array().unwrap();
    assert_eq!(rows.len(), 5);
    let places: Vec<(&str, u64)> = rows
        .iter()
        .map(|row| {
            (
                row["document_id"].as_str().unwrap(),
                row["page"].as_u64().unwrap(),
            )
        })
        .collect();
    let (first, second) = (notes(1), notes(2));
    assert_eq!(
        places,
        [
            (first.as_str(), 2),
            (first.as_str(), 3),
            (second.as_str(), 1),
            (second.as_str(), 2),
            (second.as_str(), 3),
        ]
    );
    assert_eq!(rows[2]["wording"], ALIAS);
    assert_eq!(rows[4]["kind"], "formula");
    assert!(rows.iter().all(|row| row["item_id"].is_string()));

    // Two leaves for the tools that go on, then one node for each concept next to it, by name.
    let nodes = card["nodes"].as_array().unwrap();
    assert_eq!(
        texts(&card["nodes"], "tool"),
        ["concept_items", "concept_neighbours", "concept", "concept"]
    );
    assert_eq!(
        texts(&card["nodes"], "kind"),
        ["tool", "tool", "discover", "discover"]
    );
    assert_eq!(texts(&card["nodes"], "label")[2..], [ITO, VOLATILITY]);
    assert_eq!(
        nodes[0]["arguments"],
        json!({ "concept_id": ids.black_scholes })
    );
    assert_eq!(
        nodes[3]["arguments"],
        json!({ "concept_id": ids.volatility })
    );

    // With no argument, a node for each concept, the most mentioned first.
    let root = call(client, "concept", json!({})).await;
    let root = structured(&root);
    assert_eq!(root["version"], 1);
    assert!(root.get("concept_id").is_none() && root.get("name").is_none());
    let labels = texts(&root["nodes"], "label");
    assert_eq!(labels[0], BLACK_SCHOLES);
    assert_eq!(labels.len(), 6);
    assert!(
        texts(&root["nodes"], "tool")
            .iter()
            .all(|tool| *tool == "concept")
    );
    assert!(
        texts(&root["nodes"], "kind")
            .iter()
            .all(|kind| *kind == "discover")
    );

    let both = call(
        client,
        "concept",
        json!({ "concept_id": ids.ito, "name": ITO }),
    )
    .await;
    assert!(error_text(&both).contains("not both"));
    let unknown = call(
        client,
        "concept",
        json!({ "concept_id": ConceptId::random().to_string() }),
    )
    .await;
    let text = error_text(&unknown);
    assert!(text.contains("`find_concepts`"), "{text}");
    let unnamed = call(client, "concept", json!({ "name": "no such concept" })).await;
    assert!(error_text(&unnamed).contains("`find_concepts`"));
    let bad = call(client, "concept", json!({ "concept_id": "not an id" })).await;
    assert!(error_text(&bad).contains("`concept_id` is not a concept id"));
}
