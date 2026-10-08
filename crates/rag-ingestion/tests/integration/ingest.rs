//! Runs a whole chapter through ingestion into a throwaway collection and graph, with an embedder
//! that makes up its vectors and a model that finds no concept, so nothing is billed.

use std::collections::BTreeSet;

use ocr::{PieceDetail, read_chapter};
use rag_ingestion::{Item, LabelChange, chapter_items, ingest_chapter, relabel};
use serde_json::{Value, json};

use crate::support::{
    self, SAMPLE_BOOK, StandInLlm, ThrowawayStores, assert_graph_holds_only, assert_labelled,
    points_in,
};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored ingest::"]
async fn ingest_fills_a_throwaway_collection_and_a_second_run_adds_nothing() {
    let throwaway = ThrowawayStores::new("ingest");
    let config = throwaway.config();
    let stores = throwaway.connect().await;
    let models = throwaway.models(StandInLlm::finding_nothing());
    let chapter = read_chapter(&support::sample_chapter()).unwrap();
    let items = chapter_items(&chapter);

    let summary = ingest_chapter(&support::sample_chapter(), &models, &stores)
        .await
        .unwrap();
    assert_eq!(summary.points_in_collection, items.len() as u64);

    let points = points_in(config).await;
    assert_eq!(points.len(), items.len());
    let stored_ids: BTreeSet<String> = points.iter().map(|(id, _)| id.clone()).collect();
    let item_ids: BTreeSet<String> = items.iter().map(|item| item.id.to_string()).collect();
    assert_eq!(stored_ids, item_ids);

    let stored_document = assert_graph_holds_only(&stores.graph, &items).await;
    let node_ids: BTreeSet<String> = stored_document
        .items
        .iter()
        .map(|item| item.id.clone())
        .collect();
    assert_eq!(node_ids, stored_ids, "a node has the id of its point");

    let formulas: Vec<&str> = chapter
        .pieces
        .iter()
        .filter(|piece| matches!(piece.detail, PieceDetail::Formula { .. }))
        .map(|piece| piece.content.as_str())
        .collect();
    let mut kinds = BTreeSet::new();
    for (_, payload) in &points {
        for field in [
            "doc_id",
            "doc_title",
            "page",
            "printed_page",
            "kind",
            "text",
        ] {
            assert!(
                !payload[field].is_null(),
                "a point has no {field}: {payload}"
            );
        }
        let kind = payload["kind"].as_str().unwrap();
        kinds.insert(kind.to_owned());
        match kind {
            "figure" => {
                let path = payload["image_path"]
                    .as_str()
                    .expect("a figure has its picture");
                assert!(path.ends_with("-figure.png"), "{path}");
                assert!(std::path::Path::new(path).is_file(), "{path}");
            }
            "formula" => {
                assert!(payload["image_path"].is_null());
                assert!(
                    formulas.contains(&payload["text"].as_str().unwrap()),
                    "a formula point holds the raw LaTeX"
                );
            }
            _ => assert!(payload["image_path"].is_null()),
        }
    }
    assert_eq!(
        kinds,
        ["chunk", "figure", "formula", "table"]
            .map(String::from)
            .into()
    );

    assert_label_and_cites_are_stored_only_where_the_item_has_them(&points, &items);
    for (_, payload) in &points {
        assert_eq!(payload["book"], SAMPLE_BOOK);
        assert!(
            payload.get("author").is_none() && payload.get("tags").is_none(),
            "no author and no tag was given: {payload}"
        );
    }

    let received = models.embedder.received();
    assert_eq!(received.len(), items.len(), "one input for each item");
    let with_picture: Vec<_> = received
        .iter()
        .filter(|input| input.image.is_some())
        .collect();
    assert_eq!(with_picture.len(), 3);
    for input in with_picture {
        let picture = input.image.as_ref().unwrap();
        let figure = chapter
            .pieces
            .iter()
            .find(|piece| {
                piece
                    .figure_image
                    .as_ref()
                    .map(|image| image.path.canonicalize().unwrap())
                    == Some(picture.clone())
            })
            .expect("the picture belongs to a figure of the chapter");
        assert!(
            input.text.contains(&figure.content),
            "the explanation travels with the picture"
        );
    }

    let labels = LabelChange {
        author: Some("Sheldon Natenberg".to_owned()),
        add: ["  Options ", "options", "Volatility"]
            .map(|tag| tag.parse().unwrap())
            .into(),
        remove: Vec::new(),
    };
    relabel(summary.doc_id, &labels, &stores).await.unwrap();
    let labelled = assert_labelled(
        config,
        &stores.graph,
        "Sheldon Natenberg",
        &["options", "volatility"],
    )
    .await;
    let labelled_ids: BTreeSet<String> = labelled.keys().cloned().collect();
    assert_eq!(
        labelled_ids, item_ids,
        "the points are the ones of the items"
    );

    let again = ingest_chapter(&support::sample_chapter(), &models, &stores)
        .await
        .unwrap();
    assert_eq!(again.points_in_collection, summary.points_in_collection);
    let points_after = points_in(config).await;
    let ids_after: BTreeSet<String> = points_after.into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids_after, stored_ids);
    assert_eq!(
        assert_graph_holds_only(&stores.graph, &items).await,
        stored_document,
        "a second run adds no node and no edge"
    );
    assert_eq!(
        assert_labelled(
            config,
            &stores.graph,
            "Sheldon Natenberg",
            &["options", "volatility"]
        )
        .await,
        labelled,
        "a second run keeps the labels that were set in between"
    );
    let received = models.embedder.received();
    assert_eq!(received.len(), 2 * items.len(), "labelling embeds nothing");
    assert_eq!(
        received[..items.len()],
        received[items.len()..],
        "labels do not change what is embedded"
    );

    relabel(summary.doc_id, &labels, &stores).await.unwrap();
    assert_eq!(
        assert_labelled(
            config,
            &stores.graph,
            "Sheldon Natenberg",
            &["options", "volatility"]
        )
        .await,
        labelled,
        "the same labels again change nothing"
    );

    let other = LabelChange {
        author: Some("Another Author".to_owned()),
        add: vec!["Greeks".parse().unwrap()],
        remove: Vec::new(),
    };
    relabel(summary.doc_id, &other, &stores).await.unwrap();
    let changed = assert_labelled(
        config,
        &stores.graph,
        "Another Author",
        &["greeks", "options", "volatility"],
    )
    .await;
    assert_eq!(changed.keys().cloned().collect::<BTreeSet<_>>(), stored_ids);
    assert_eq!(
        assert_graph_holds_only(&stores.graph, &items).await,
        stored_document,
        "other labels add no node and no edge"
    );
}

/// A point holds `label` and `cites` where its item has them and leaves the keys out everywhere
/// else, so a point stored before the two fields existed reads the same as a new one.
fn assert_label_and_cites_are_stored_only_where_the_item_has_them(
    points: &[(String, Value)],
    items: &[Item],
) {
    let (mut with_label, mut with_cites) = (0, 0);
    for (id, payload) in points {
        let item = items
            .iter()
            .find(|item| item.id.to_string() == *id)
            .expect("every point is an item of the chapter");
        match &item.payload.label {
            Some(label) => {
                assert_eq!(payload["label"], *label);
                with_label += 1;
            }
            None => assert!(payload.get("label").is_none(), "{payload}"),
        }
        if item.payload.cites.is_empty() {
            assert!(payload.get("cites").is_none(), "{payload}");
        } else {
            assert_eq!(payload["cites"], json!(item.payload.cites));
            with_cites += 1;
        }
    }
    assert!(with_label > 0, "the sample chapter has labelled items");
    assert!(with_cites > 0, "the sample chapter has chunks that cite");
}
