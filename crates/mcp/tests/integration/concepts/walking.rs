//! The tools that walk the graph from a concept: its relations, its items and the chain between
//! two concepts.

use super::{Ids, edge_of_kind, kinds, notes, texts};
use crate::support::concept_graph::{
    BLACK_SCHOLES, BLACK_SCHOLES_DEFINITION, ITO, LONG_STRADDLE, STOCHASTIC_CALCULUS, VOLATILITY,
    item_saying,
};
use crate::support::{Client, call, error_text, sample_content_folder, structured};
use serde_json::{Value, json};

pub(super) async fn concept_neighbours_follow_kind_direction_and_depth(client: &Client, ids: &Ids) {
    let around = async |concept: &str, arguments: Value| {
        let mut arguments = arguments;
        arguments["concept_id"] = json!(concept);
        let result = call(client, "concept_neighbours", arguments).await;
        structured(&result).clone()
    };

    let near = around(&ids.black_scholes, json!({})).await;

    assert_eq!(near["concept_id"], ids.black_scholes.as_str());
    assert_eq!(near["truncated"], false);
    assert_eq!(kinds(&near["edges"]), ["ASSUMES", "DERIVED_FROM"]);
    // The edge that the Black–Scholes model leaves for Volatility was stated by the item of the
    // first notes chapter that calls Volatility its one input, and the edge for Itô's lemma by the
    // item of the second chapter that names the lemma.
    let assumes = edge_of_kind(&near["edges"], "ASSUMES");
    let volatility_item = item_saying(
        &sample_content_folder().join("quanty-sample-notes/chapter-1"),
        "Volatility* is the one input",
    );
    assert_eq!(assumes["from"], ids.black_scholes.as_str());
    assert_eq!(assumes["to"], ids.volatility.as_str());
    assert_eq!(
        assumes["stated_by"],
        json!({ "item_id": volatility_item.id.to_string(), "document_id": notes(1), "page": 3 })
    );
    let derived = edge_of_kind(&near["edges"], "DERIVED_FROM");
    assert_eq!(derived["to"], ids.ito.as_str());
    assert_eq!(derived["stated_by"]["document_id"], notes(2).as_str());
    assert_eq!(derived["stated_by"]["page"], 2);
    assert_eq!(
        texts(&near["concepts"], "name"),
        [BLACK_SCHOLES, ITO, VOLATILITY]
    );
    assert_eq!(near["concepts"][0]["definition"], BLACK_SCHOLES_DEFINITION);

    let only_assumes = around(&ids.black_scholes, json!({ "kinds": ["ASSUMES"] })).await;
    assert_eq!(kinds(&only_assumes["edges"]), ["ASSUMES"]);
    let leaving = around(&ids.black_scholes, json!({ "direction": "out" })).await;
    assert_eq!(leaving["edges"].as_array().unwrap().len(), 2);
    let reaching = around(&ids.black_scholes, json!({ "direction": "in" })).await;
    assert_eq!(reaching["edges"], json!([]));
    assert_eq!(texts(&reaching["concepts"], "name"), [BLACK_SCHOLES]);
    let reaching_ito = around(&ids.ito, json!({ "direction": "in" })).await;
    assert_eq!(
        kinds(&reaching_ito["edges"]),
        ["DERIVED_FROM", "GENERALISES"]
    );
    let leaving_ito = around(&ids.ito, json!({ "direction": "out" })).await;
    assert_eq!(leaving_ito["edges"], json!([]));

    // The second step starts from the concepts the first step reached, so it adds the relation of
    // Stochastic calculus, and the relations it found twice come once.
    let further = around(&ids.black_scholes, json!({ "depth": 2 })).await;
    assert_eq!(
        kinds(&further["edges"]),
        ["ASSUMES", "DERIVED_FROM", "GENERALISES"]
    );
    assert_eq!(
        texts(&further["concepts"], "name"),
        [BLACK_SCHOLES, ITO, STOCHASTIC_CALCULUS, VOLATILITY]
    );
    let cut = around(&ids.black_scholes, json!({ "limit": 1 })).await;
    assert_eq!(cut["edges"].as_array().unwrap().len(), 1);
    assert_eq!(cut["truncated"], true);
    let cut_further = around(&ids.black_scholes, json!({ "depth": 2, "limit": 2 })).await;
    assert_eq!(cut_further["edges"].as_array().unwrap().len(), 2);
    assert_eq!(cut_further["truncated"], true);

    for (arguments, words) in [
        (json!({ "direction": "sideways" }), "`direction`"),
        (
            json!({ "kinds": ["LIKES"] }),
            "`LIKES` is not a kind of relation",
        ),
    ] {
        let refused = call(
            client,
            "concept_neighbours",
            with_concept(arguments, &ids.ito),
        )
        .await;
        let text = error_text(&refused);
        assert!(text.contains(words), "{text}");
    }
}

pub(super) async fn concept_items_come_a_page_at_a_time(client: &Client, ids: &Ids) {
    let page = async |arguments: Value| {
        let mut arguments = arguments;
        arguments["concept_id"] = json!(ids.black_scholes);
        let result = call(client, "concept_items", arguments).await;
        structured(&result).clone()
    };

    let everything = page(json!({})).await;
    assert_eq!(everything["concept_id"], ids.black_scholes.as_str());
    assert_eq!(everything["items"].as_array().unwrap().len(), 5);
    assert!(everything.get("next_cursor").is_none());

    // Three rows, then the cursor of the result gives the last two.
    let first = page(json!({ "limit": 3 })).await;
    assert_eq!(
        first["items"],
        json!(everything["items"].as_array().unwrap()[..3])
    );
    let cursor = first["next_cursor"].as_str().expect("there is a next page");
    let last = page(json!({ "limit": 3, "cursor": cursor })).await;
    assert_eq!(
        last["items"],
        json!(everything["items"].as_array().unwrap()[3..])
    );
    assert!(last.get("next_cursor").is_none());

    let of_second = page(json!({ "document": notes(2) })).await;
    assert_eq!(
        texts(&of_second["items"], "document_id"),
        [notes(2), notes(2), notes(2)]
    );
    let formulas = page(json!({ "kind": "formula" })).await;
    assert_eq!(formulas["items"].as_array().unwrap().len(), 1);
    let figures = page(json!({ "kind": "figure" })).await;
    assert_eq!(figures["items"], json!([]));
    let chunks_of_first = page(json!({ "kind": "chunk", "document": notes(1) })).await;
    assert_eq!(chunks_of_first["items"].as_array().unwrap().len(), 2);

    for (arguments, words) in [
        (json!({ "cursor": "the end" }), "`cursor` is \"the end\""),
        (json!({ "kind": "poem" }), "`kind` is not valid"),
    ] {
        let refused = call(client, "concept_items", with_concept(arguments, &ids.ito)).await;
        let text = error_text(&refused);
        assert!(text.contains(words), "{text}");
    }
}

pub(super) async fn concept_path_crosses_edges_either_way_and_is_empty_when_there_is_none(
    client: &Client,
    ids: &Ids,
) {
    // A name for one end and an id for the other. The chain goes with the arrow from Stochastic
    // calculus to Itô's lemma, against it to the Black–Scholes model, and with it to Volatility.
    let path = call(
        client,
        "concept_path",
        json!({ "from": "stochastic CALCULUS", "to": ids.volatility }),
    )
    .await;

    let found = structured(&path);
    assert_eq!(found["from"]["name"], STOCHASTIC_CALCULUS);
    assert_eq!(found["to"]["concept_id"], ids.volatility.as_str());
    assert_eq!(
        texts(&found["edges"], "kind"),
        ["GENERALISES", "DERIVED_FROM", "ASSUMES"]
    );
    let edges = found["edges"].as_array().unwrap();
    assert_eq!(
        (&edges[0]["from"], &edges[0]["to"]),
        (&json!(ids.stochastic), &json!(ids.ito))
    );
    assert_eq!(
        (&edges[1]["from"], &edges[1]["to"]),
        (&json!(ids.black_scholes), &json!(ids.ito))
    );
    assert_eq!(
        (&edges[2]["from"], &edges[2]["to"]),
        (&json!(ids.black_scholes), &json!(ids.volatility))
    );
    assert!(edges.iter().all(|edge| edge["stated_by"]["page"].is_u64()));
    assert_eq!(
        texts(&found["concepts"], "name"),
        [BLACK_SCHOLES, ITO, STOCHASTIC_CALCULUS, VOLATILITY]
    );

    // Too short a cap, and a concept with no relation, give an empty chain and not an error.
    let capped = call(
        client,
        "concept_path",
        json!({ "from": ids.stochastic, "to": ids.volatility, "max_depth": 2 }),
    )
    .await;
    assert_eq!(structured(&capped)["edges"], json!([]));
    assert_eq!(structured(&capped)["concepts"], json!([]));
    assert_eq!(structured(&capped)["to"]["name"], VOLATILITY);
    let alone = call(
        client,
        "concept_path",
        json!({ "from": ids.black_scholes, "to": LONG_STRADDLE }),
    )
    .await;
    assert_eq!(structured(&alone)["edges"], json!([]));
    assert_eq!(structured(&alone)["to"]["name"], LONG_STRADDLE);
    let short = call(
        client,
        "concept_path",
        json!({ "from": ids.black_scholes, "to": ids.ito, "max_depth": 1 }),
    )
    .await;
    assert_eq!(structured(&short)["edges"].as_array().unwrap().len(), 1);

    let same = call(
        client,
        "concept_path",
        json!({ "from": ids.ito, "to": ITO }),
    )
    .await;
    let text = error_text(&same);
    assert!(text.contains("the same concept"), "{text}");
}

pub(super) fn with_concept(mut arguments: Value, concept: &str) -> Value {
    arguments["concept_id"] = json!(concept);
    arguments
}
