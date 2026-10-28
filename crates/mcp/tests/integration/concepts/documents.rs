//! The tools that read documents and items: the concepts two documents share, the items nearest
//! to an item, and the concepts of one document.

use super::{NOTES_1_TITLE, NOTES_2_TITLE, notes, texts};
use crate::support::concept_graph::{
    BLACK_SCHOLES, ITO, LONG_STRADDLE, NEAR_THE_FIGURE, NEAR_THE_FIGURE_SCORE, STOCHASTIC_CALCULUS,
    TIME_VALUE, VOLATILITY, figure_of_page_5, item_saying,
};
use crate::support::{
    Client, Library, StandInServices, call, connect, error_text, sample_content_folder,
    sample_document_id, structured,
};
use rag_core::{DocId, EmbedError};
use serde_json::json;

pub(super) async fn compare_documents_splits_shared_from_only_in(client: &Client) {
    let both = call(
        client,
        "compare_documents",
        json!({ "document_ids": [notes(1), notes(2)] }),
    )
    .await;

    let compared = structured(&both);
    assert_eq!(
        compared["documents"],
        json!([
            { "document_id": notes(1), "title": NOTES_1_TITLE },
            { "document_id": notes(2), "title": NOTES_2_TITLE },
        ])
    );
    assert_eq!(
        texts(&compared["shared"], "name"),
        [BLACK_SCHOLES, VOLATILITY]
    );
    assert_eq!(
        compared["shared"][0]["mentions"],
        json!({ notes(1): 2, notes(2): 3 })
    );
    assert_eq!(
        compared["shared"][1]["mentions"],
        json!({ notes(1): 1, notes(2): 1 })
    );
    assert_eq!(texts(&compared["only_in"][notes(1)], "name"), [TIME_VALUE]);
    assert_eq!(compared["only_in"][notes(1)][0]["mentions"], 1);
    assert_eq!(
        texts(&compared["only_in"][notes(2)], "name"),
        [ITO, STOCHASTIC_CALCULUS]
    );
    assert_eq!(compared["truncated"], false);

    // The sample chapter has a concept of its own, and every document is a key of `only_in`.
    let sample = sample_document_id().to_string();
    let three = call(
        client,
        "compare_documents",
        json!({ "document_ids": [sample, notes(1), notes(2)] }),
    )
    .await;
    let three = structured(&three);
    assert_eq!(
        texts(&three["only_in"][sample.as_str()], "name"),
        [LONG_STRADDLE]
    );
    assert_eq!(three["only_in"].as_object().unwrap().len(), 3);

    let cut = call(
        client,
        "compare_documents",
        json!({ "document_ids": [notes(1), notes(2)], "limit": 1 }),
    )
    .await;
    assert_eq!(structured(&cut)["shared"].as_array().unwrap().len(), 1);
    assert_eq!(structured(&cut)["truncated"], true);

    let twice = call(
        client,
        "compare_documents",
        json!({ "document_ids": [notes(1), notes(1)] }),
    )
    .await;
    let text = error_text(&twice);
    assert!(
        text.contains("from 2 to 10 different document ids"),
        "{text}"
    );
    let unknown = DocId::from_source_sha256("a pdf that was never stored").to_string();
    let missing = call(
        client,
        "compare_documents",
        json!({ "document_ids": [notes(1), unknown] }),
    )
    .await;
    let text = error_text(&missing);
    assert!(
        text.contains(&unknown) && text.contains("`list_documents`"),
        "{text}"
    );
}

pub(super) async fn similar_items_embeds_nothing(library: &Library) {
    // The embedder of these services cannot be made, so any call that embeds fails.
    let services = StandInServices::new().with_failing_embedder(|| EmbedError::MissingApiKey);
    let client = connect(library.server_over(&sample_content_folder(), services)).await;
    let figure = figure_of_page_5();
    let near = item_saying(
        &sample_content_folder().join("quanty-sample-notes/chapter-1"),
        NEAR_THE_FIGURE,
    );

    let result = call(
        &client,
        "similar_items",
        json!({ "item_id": figure.id.to_string(), "limit": 3 }),
    )
    .await;

    let similar = structured(&result);
    assert_eq!(similar["item_id"], figure.id.to_string().as_str());
    let items = similar["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0]["item_id"], near.id.to_string().as_str());
    assert_eq!(items[0]["document_id"], notes(1).as_str());
    assert_eq!(items[0]["document"], NOTES_1_TITLE);
    assert_eq!(items[0]["page"], near.payload.page);
    assert_eq!(items[0]["kind"], "chunk");
    let score = items[0]["score"].as_f64().unwrap();
    assert!(
        (score - f64::from(NEAR_THE_FIGURE_SCORE)).abs() < 0.01,
        "{score}"
    );
    assert!(
        items
            .iter()
            .all(|item| item["item_id"] != figure.id.to_string().as_str())
    );

    let formulas = call(
        &client,
        "similar_items",
        json!({ "item_id": figure.id.to_string(), "kind": "formula" }),
    )
    .await;
    let formulas = structured(&formulas)["items"].as_array().unwrap().clone();
    assert!(!formulas.is_empty() && formulas.iter().all(|item| item["kind"] == "formula"));

    let unknown = rag_core::ItemId::new(sample_document_id(), rag_core::ItemKind::Code, 999);
    let missing = call(
        &client,
        "similar_items",
        json!({ "item_id": unknown.to_string() }),
    )
    .await;
    let text = error_text(&missing);
    assert!(text.contains("no stored item has the id"), "{text}");
}

pub(super) async fn document_concepts_adds_ids_limits_and_counts(client: &Client) {
    let second = notes(2);
    let whole = call(
        client,
        "document_concepts",
        json!({ "document_id": second }),
    )
    .await;

    let whole = structured(&whole);
    assert_eq!(whole["title"], NOTES_2_TITLE);
    assert!(whole.get("page").is_none());
    assert_eq!(
        texts(&whole["concepts"], "name"),
        [BLACK_SCHOLES, ITO, STOCHASTIC_CALCULUS, VOLATILITY]
    );
    assert!(whole["concepts"][0]["concept_id"].is_string());
    assert!(whole["concepts"][0].get("mentions").is_none());

    // The same names as `chapter_concepts` gives.
    let old = call(client, "chapter_concepts", json!({ "document_id": second })).await;
    assert_eq!(
        texts(&structured(&old)["concepts"], "name"),
        texts(&whole["concepts"], "name")
    );

    let counted = call(
        client,
        "document_concepts",
        json!({ "document_id": second, "limit": 2, "with_counts": true }),
    )
    .await;
    let counted = structured(&counted);
    assert_eq!(texts(&counted["concepts"], "name"), [BLACK_SCHOLES, ITO]);
    assert_eq!(counted["concepts"][0]["mentions"], 3);
    assert_eq!(counted["concepts"][1]["mentions"], 2);

    // On page 2 the Black–Scholes model is in one item, and still counts the items of the whole
    // document. Itô's lemma and Stochastic calculus are in two items there, so they come first.
    let on_page = call(
        client,
        "document_concepts",
        json!({ "document_id": second, "page": 2, "with_counts": true }),
    )
    .await;
    let on_page = structured(&on_page);
    assert_eq!(on_page["page"], 2);
    assert_eq!(
        texts(&on_page["concepts"], "name"),
        [ITO, STOCHASTIC_CALCULUS, BLACK_SCHOLES]
    );
    assert_eq!(on_page["concepts"][2]["mentions"], 3);

    let zero = call(
        client,
        "document_concepts",
        json!({ "document_id": second, "page": 0 }),
    )
    .await;
    assert!(error_text(&zero).contains("pages count from 1"));
}
