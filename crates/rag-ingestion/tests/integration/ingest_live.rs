//! Runs the real `rag-ingest` command twice on the sample chapter, against the real Gemini API
//! and a throwaway collection of the local Qdrant, because only that shows that the binary
//! reads its key, finds its collection and stores points that a second run overwrites.

use std::collections::BTreeSet;
use std::process::Command;

use ocr::read_chapter;
use rag_ingestion::chapter_items;

use crate::support::{self, ThrowawayCollection, points_in};

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
    let throwaway = ThrowawayCollection::new("ingest-live");
    let config = throwaway.config();
    let expected_items = chapter_items(&read_chapter(&support::sample_chapter()).unwrap()).len();

    let run = || {
        Command::new(env!("CARGO_BIN_EXE_rag-ingest"))
            .arg(support::sample_chapter())
            .env("QDRANT_ITEMS_COLLECTION", &config.items_collection)
            .output()
            .expect("the rag-ingest binary should start")
    };

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
}
