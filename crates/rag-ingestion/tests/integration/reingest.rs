//! Ingests one document twice into throwaway stores, the second time cut into other items whose
//! answers name another concept, to show that nothing of the first run is left. Nothing is billed.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use graph::testing::stored_concept_graph;
use ocr::read_chapter;
use rag_ingestion::{chapter_items, ingest_chapter, items_of_ingested_document};
use serde_json::{Value, json};

use crate::support::{self, StandInLlm, ThrowawayStores, assert_document_stored, points_in};

fn write_json(file: &Path, value: &Value) {
    fs::write(file, serde_json::to_string_pretty(value).unwrap()).unwrap();
}

/// A converted chapter of one page with each text under a heading of its own, so each text is an
/// item. Every chapter written here has the same source, so it is the same document.
fn write_chapter(folder: &Path, texts: &[&str]) {
    let page = folder.join("page-num-1");
    fs::create_dir_all(&page).unwrap();
    write_json(
        &folder.join("chapter.json"),
        &json!({
            "format-version": 1,
            "media-title": "Notes Cut Twice",
            "name": { "chapter": { "number": 1, "name": "One Source" } },
            "source-file": "chapter-1-one-source.pdf",
            "source-sha256": "c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2",
            "page-count": 1,
            "finished": true,
        }),
    );
    let mut pieces = Vec::new();
    for (index, text) in texts.iter().enumerate() {
        let heading = 2 * index + 1;
        let body = heading + 1;
        pieces.push(json!({
            "number": heading,
            "file": format!("{heading:02}-heading.md"),
            "kind": "heading",
            "rank": 1,
            "printed-number": null,
        }));
        pieces.push(json!({
            "number": body,
            "file": format!("{body:02}-text.md"),
            "kind": "text",
            "cites": [],
        }));
        fs::write(page.join(format!("{heading:02}-heading.md")), "Part\n").unwrap();
        fs::write(page.join(format!("{body:02}-text.md")), format!("{text}\n")).unwrap();
    }
    write_json(
        &page.join("page.json"),
        &json!({
            "format-version": 1,
            "page-position": 1,
            "pieces": pieces,
            "relationships": [],
        }),
    );
}

fn finding(name: &'static str) -> StandInLlm {
    StandInLlm::replying(move |_, _| {
        Ok(json!({
            "concepts": [{ "name": name, "definition": "what the text is about" }],
            "relations": [],
        }))
    })
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored reingest::"]
async fn a_document_ingested_again_in_other_items_keeps_nothing_of_the_first_run() {
    let throwaway = ThrowawayStores::new("reingest");
    let stores = throwaway.connect().await;
    let first = throwaway.temporary_folder().join("first");
    let second = throwaway.temporary_folder().join("second");
    write_chapter(&first, &["Calls gain.", "Puts gain.", "Both decay."]);
    write_chapter(
        &second,
        &[
            "Calls gain when prices rise.",
            "Puts gain when prices fall.",
        ],
    );
    let first_items = chapter_items(&read_chapter(&first).unwrap());
    let items = chapter_items(&read_chapter(&second).unwrap());
    let item_ids: BTreeSet<String> = items.iter().map(|item| item.id.to_string()).collect();
    assert!(
        first_items
            .iter()
            .any(|item| !item_ids.contains(&item.id.to_string())),
        "the first cut must have an item that the second does not have"
    );

    ingest_chapter(
        support::chapter_at(&first),
        &throwaway.models(finding("volatility")),
        &stores,
    )
    .await
    .unwrap();
    let summary = ingest_chapter(
        support::chapter_at(&second),
        &throwaway.models(finding("interest rate")),
        &stores,
    )
    .await
    .unwrap();

    let point_ids: BTreeSet<String> = points_in(throwaway.config())
        .await
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert_eq!(point_ids, item_ids, "no point of the first run is left");
    assert_document_stored(&stores.graph, &items).await;
    let mentions: BTreeSet<(String, String)> = stored_concept_graph(&stores.graph)
        .await
        .mentions
        .into_iter()
        .map(|mention| (mention.item, mention.concept))
        .collect();
    let expected: BTreeSet<(String, String)> = item_ids
        .iter()
        .map(|id| (id.clone(), "interest rate".to_owned()))
        .collect();
    assert_eq!(mentions, expected, "no mention of the first run is left");
    assert_eq!(
        items_of_ingested_document(summary.doc_id, &stores)
            .await
            .unwrap(),
        Some(items.len() as u64),
        "the document counts as ingested whole"
    );
}
