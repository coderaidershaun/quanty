//! Runs `rag-ingest delete-document` as a person would, against a throwaway collection of the
//! local Qdrant and a throwaway graph of the local FalkorDB, because only that shows that the
//! command removes one document from both stores and leaves the others alone.

use std::collections::BTreeSet;
use std::process::Output;

use graph::testing::{GraphSize, size, stored_document};
use ocr::read_chapter;
use rag_ingestion::{DeleteSummary, Item, chapter_items, ingest_chapter};

use crate::support::{
    self, StandInEmbedder, ThrowawayStores, assert_graph_holds_only, points_in,
    size_of_one_document,
};

fn items_of(chapter_folder: &std::path::Path) -> Vec<Item> {
    chapter_items(&read_chapter(chapter_folder).unwrap())
}

fn delete_document(throwaway: &ThrowawayStores, id: &str) -> Output {
    throwaway.rag_ingest(["delete-document", id])
}

fn stdout_lines(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect()
}

fn stderr_text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The lines the command prints for this summary, written out here so that a change to what a
/// person reads is seen.
fn printed_lines(summary: &DeleteSummary) -> [String; 3] {
    [
        format!("document id: {}", summary.doc_id),
        format!(
            "points removed from collection {}: {}",
            summary.collection, summary.points_removed
        ),
        format!(
            "nodes removed from the graph: {} (the document and its items)",
            summary.nodes_removed
        ),
    ]
}

async fn point_ids(throwaway: &ThrowawayStores) -> BTreeSet<String> {
    points_in(throwaway.config())
        .await
        .into_iter()
        .map(|(id, _)| id)
        .collect()
}

fn ids_of(items: &[Item]) -> BTreeSet<String> {
    items.iter().map(|item| item.id.to_string()).collect()
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored delete::"]
async fn delete_document_removes_one_document_from_both_stores_and_leaves_the_other_whole() {
    let throwaway = ThrowawayStores::new("delete");
    let collection = throwaway.config().items_collection.clone();
    let stores = throwaway.connect().await;
    let embedder = StandInEmbedder::default();
    let items_a = items_of(&support::intuition_chapter());
    let items_b = items_of(&support::in_depth_chapter());
    let (a, b) = (items_a.len() as u64, items_b.len() as u64);
    let first = ingest_chapter(&support::intuition_chapter(), &embedder, &stores)
        .await
        .unwrap();
    let second = ingest_chapter(&support::in_depth_chapter(), &embedder, &stores)
        .await
        .unwrap();
    assert_ne!(first.doc_id, second.doc_id);
    assert_eq!(second.points_in_collection, a + b);

    let output = delete_document(&throwaway, &first.doc_id.to_string());
    assert!(output.status.success(), "{}", stderr_text(&output));
    assert_eq!(
        stdout_lines(&output),
        printed_lines(&DeleteSummary {
            doc_id: first.doc_id,
            collection: collection.clone(),
            points_removed: a,
            nodes_removed: a + 1,
        })
    );
    assert!(stored_document(&stores.graph, first.doc_id).await.is_none());
    assert_graph_holds_only(&stores.graph, &items_b).await;
    assert_eq!(point_ids(&throwaway).await, ids_of(&items_b));

    // A delete that stopped half way, after the points and before the nodes, is finished by
    // running it again.
    let removed = stores.items.delete_document(second.doc_id).await.unwrap();
    assert_eq!(removed, b);
    assert_eq!(size(&stores.graph).await, size_of_one_document(b as usize));
    let output = delete_document(&throwaway, &second.doc_id.to_string());
    assert!(output.status.success(), "{}", stderr_text(&output));
    assert_eq!(
        stdout_lines(&output),
        printed_lines(&DeleteSummary {
            doc_id: second.doc_id,
            collection,
            points_removed: 0,
            nodes_removed: b + 1,
        })
    );
    assert!(
        stored_document(&stores.graph, second.doc_id)
            .await
            .is_none()
    );
    assert_eq!(size(&stores.graph).await, GraphSize { nodes: 0, edges: 0 });
    assert!(point_ids(&throwaway).await.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored delete::"]
async fn delete_document_refuses_an_id_that_is_not_a_document_id() {
    let throwaway = ThrowawayStores::new("delete-refuse");
    let stores = throwaway.connect().await;
    let embedder = StandInEmbedder::default();
    let items = items_of(&support::intuition_chapter());
    ingest_chapter(&support::intuition_chapter(), &embedder, &stores)
        .await
        .unwrap();

    let item_id = items[0].id.to_string();
    let output = delete_document(&throwaway, &item_id);
    assert_eq!(output.status.code(), Some(1));
    let message = stderr_text(&output);
    assert!(
        message.contains(&format!(
            "nothing is stored under the document id {item_id}"
        )),
        "{message}"
    );

    let output = delete_document(&throwaway, "not-a-document-id");
    assert_eq!(output.status.code(), Some(2));
    let message = stderr_text(&output);
    assert!(message.contains("is not a document id"), "{message}");

    assert_graph_holds_only(&stores.graph, &items).await;
    assert_eq!(point_ids(&throwaway).await, ids_of(&items));
}
