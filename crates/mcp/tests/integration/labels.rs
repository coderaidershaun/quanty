//! Checks `edit_media` and `edit_document_tags` over throwaway stores that hold the three
//! committed chapters: the new labels show in the list of documents and filter a search.

use serde_json::{Value, json};

use crate::support::{
    Library, QUESTION, call, connect, error_text, sample_document_id, structured,
};

/// The `document_id` of every result of a search with these filters, from the library's server.
async fn found_by(client: &crate::support::Client, filters: Value) -> Vec<String> {
    let mut arguments = json!({ "question": QUESTION });
    arguments
        .as_object_mut()
        .unwrap()
        .extend(filters.as_object().unwrap().clone());
    let result = call(client, "search", arguments).await;
    structured(&result)["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|found| found["document_id"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p mcp --test integration -- --ignored labels::"]
async fn the_labels_of_a_media_and_a_document_change_in_both_stores() {
    let library = Library::ingested("mcp-labels").await;
    let client = connect(library.server()).await;
    let sample = sample_document_id().to_string();

    let edited = call(
        &client,
        "edit_media",
        json!({
            "media": "option volatility AND pricing",
            "category": "other",
            "authors": ["Someone Else", " ", "Someone Else"],
            "tags": ["Greeks", "volatility"],
        }),
    )
    .await;

    assert_eq!(
        structured(&edited),
        &json!({
            "media": "Option Volatility and Pricing",
            "category": "other",
            "authors": ["Someone Else"],
            "tags": ["greeks", "volatility"],
            "documents": 1,
        })
    );
    let listed = call(&client, "list_documents", json!({})).await;
    let first = &structured(&listed)["documents"][0];
    assert_eq!(first["document_id"], sample.as_str());
    assert_eq!(first["category"], "other");
    assert_eq!(first["authors"], json!(["Someone Else"]));
    assert_eq!(first["media_tags"], json!(["greeks", "volatility"]));
    let by_new_labels = found_by(
        &client,
        json!({ "tags": ["greeks"], "author": "Someone Else", "category": "other" }),
    )
    .await;
    assert!(!by_new_labels.is_empty());
    assert!(by_new_labels.iter().all(|found| *found == sample));
    let by_old_label = found_by(&client, json!({ "tags": ["options"] })).await;
    assert_eq!(
        by_old_label,
        Vec::<String>::new(),
        "the old tag finds nothing"
    );

    let added = call(
        &client,
        "edit_document_tags",
        json!({ "document_id": sample, "add": ["Chapter-Note", "greeks"] }),
    )
    .await;

    assert_eq!(
        structured(&added),
        &json!({
            "document_id": sample,
            "tags": ["chapter-note", "greeks"],
            "media_tags": ["greeks", "volatility"],
        })
    );
    let listed = call(&client, "list_documents", json!({})).await;
    assert_eq!(
        structured(&listed)["documents"][0]["tags"],
        json!(["chapter-note", "greeks"])
    );
    let by_own_tag = found_by(&client, json!({ "tags": ["chapter-note"] })).await;
    assert!(!by_own_tag.is_empty());
    assert!(by_own_tag.iter().all(|found| *found == sample));

    let removed = call(
        &client,
        "edit_document_tags",
        json!({ "document_id": sample, "remove": ["chapter-note", "never-had"] }),
    )
    .await;

    assert_eq!(structured(&removed)["tags"], json!(["greeks"]));
    let by_removed_tag = found_by(&client, json!({ "tags": ["chapter-note"] })).await;
    assert_eq!(by_removed_tag, Vec::<String>::new());

    let cleared = call(
        &client,
        "edit_media",
        json!({ "media": "Option Volatility and Pricing", "tags": [] }),
    )
    .await;

    assert_eq!(structured(&cleared)["tags"], json!([]));
    assert_eq!(
        structured(&cleared)["category"],
        "other",
        "a label left out stays"
    );
    let unknown = call(
        &client,
        "edit_media",
        json!({ "media": "No Such Media", "tags": ["a"] }),
    )
    .await;
    let text = error_text(&unknown);
    assert!(text.contains("No Such Media"), "{text}");
    assert!(text.contains("`list_documents`"), "{text}");
}
