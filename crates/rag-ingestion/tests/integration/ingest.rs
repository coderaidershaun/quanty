//! Runs a whole chapter through ingestion into a throwaway collection of the local Qdrant and a
//! throwaway graph of the local FalkorDB, with an embedder that makes up its vectors and a
//! language model that finds no concept, so nothing is billed.

use std::collections::BTreeSet;

use ocr::{PieceDetail, read_chapter};
use rag_ingestion::{chapter_items, ingest_chapter};

use crate::support::{self, StandInLlm, ThrowawayStores, assert_graph_holds_only, points_in};

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
}
