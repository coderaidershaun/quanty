//! Runs the real `rag-ingest` command twice on the sample chapter, against the real Gemini API,
//! a throwaway collection of the local Qdrant and a throwaway graph of the local FalkorDB,
//! because only that shows that the binary reads its key, finds its collection and its graph,
//! and stores points and nodes that a second run overwrites. The `claude` program is a stand-in
//! that finds no concept, so only Gemini bills.

use std::collections::BTreeSet;

use graph::FalkorGraph;
use graph::testing::size;
use ocr::read_chapter;
use rag_ingestion::chapter_items;

use crate::support::{self, ThrowawayStores, points_in, size_of_one_document};

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
}
