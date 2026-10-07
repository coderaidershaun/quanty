//! Runs the real `rag-ingest` command twice on the sample chapter, and once on the sample
//! picture, against the real Gemini API, a throwaway collection of the local Qdrant and a
//! throwaway graph of the local FalkorDB, because only that shows that the binary reads its key,
//! finds its collection and its graph, stores points and nodes that a second run overwrites, and
//! embeds a picture with its text. The `claude` program is a stand-in that finds no concept, and
//! the picture is converted in this process with a stand-in for Sonnet, so only Gemini bills.

use std::collections::BTreeSet;
use std::ffi::OsString;

use graph::FalkorGraph;
use graph::testing::size;
use ocr::convert::convert_image_with;
use ocr::read_chapter;
use rag_ingestion::chapter_items;

use crate::support::{
    self, StandInImageServices, ThrowawayStores, points_in, size_of_one_document,
};

const NOTE: &str = "A chart from a book on option trading.";

const RUN_COMMAND: &str = "set -a; . ./.env; set +a; REX_PROD_API=true cargo test -p rag-ingestion --test integration -- --ignored ingest_live::";

fn require_prod_api() {
    assert_eq!(
        std::env::var("REX_PROD_API").as_deref(),
        Ok("true"),
        "this test calls the real Gemini API; run it with: {RUN_COMMAND}"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "calls the real Gemini API and spends API credit; run with: set -a; . ./.env; set +a; REX_PROD_API=true cargo test -p rag-ingestion --test integration -- --ignored ingest_live::"]
async fn rag_ingest_fills_a_throwaway_collection_twice_live() {
    require_prod_api();
    let throwaway = ThrowawayStores::new("ingest-live");
    let config = throwaway.config();
    let expected_items = chapter_items(&read_chapter(&support::sample_chapter()).unwrap()).len();

    let run = || throwaway.rag_ingest([support::sample_chapter()]);

    let first = run();
    assert!(
        first.status.success(),
        "first run failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let points = points_in(config).await;
    assert_eq!(points.len(), expected_items);
    let kinds: BTreeSet<&str> = points
        .iter()
        .filter_map(|(_, payload)| payload["kind"].as_str())
        .collect();
    assert_eq!(
        kinds,
        BTreeSet::from(["chunk", "figure", "formula", "table"])
    );
    let ids: BTreeSet<&String> = points.iter().map(|(id, _)| id).collect();
    let graph = FalkorGraph::connect(config).await.unwrap();
    let graph_size = size_of_one_document(expected_items);
    assert_eq!(size(&graph).await, graph_size);

    let second = run();
    assert!(
        second.status.success(),
        "second run failed: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    let points_again = points_in(config).await;
    assert_eq!(
        points_again.len(),
        expected_items,
        "a second run adds nothing"
    );
    let ids_again: BTreeSet<&String> = points_again.iter().map(|(id, _)| id).collect();
    assert_eq!(ids_again, ids);
    assert_eq!(size(&graph).await, graph_size, "a second run adds nothing");

    // The command ingests a picture that was converted before, so no model reads it: Gemini
    // embeds the picture together with its explanation and the note.
    convert_image_with(
        &support::sample_picture(),
        &config.content_folder,
        &StandInImageServices::default(),
    )
    .await
    .unwrap();
    let picture_run = throwaway.rag_ingest([
        support::sample_picture().into_os_string(),
        OsString::from("--note"),
        OsString::from(NOTE),
    ]);
    assert!(
        picture_run.status.success(),
        "the picture run failed: {}",
        String::from_utf8_lossy(&picture_run.stderr)
    );
    let printed = String::from_utf8_lossy(&picture_run.stdout);
    assert!(
        printed.contains("document: volatility-surface.png"),
        "{printed}"
    );
    let points_with_picture = points_in(config).await;
    assert_eq!(points_with_picture.len(), expected_items + 1);
    let new_points: Vec<_> = points_with_picture
        .iter()
        .filter(|(id, _)| !ids.contains(id))
        .collect();
    assert_eq!(new_points.len(), 1, "one point more");
    let payload = &new_points[0].1;
    assert_eq!(payload["kind"], "figure");
    assert_eq!(payload["doc_title"], "volatility-surface.png");
    assert!(
        payload["text"].as_str().unwrap().ends_with(NOTE),
        "{payload}"
    );
}
