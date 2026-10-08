//! Runs `rag-ingest delete-document` against throwaway stores, to show that it removes one
//! document from both and leaves the others alone. Concepts belong to no document, so they stay.

use std::collections::BTreeSet;
use std::process::Output;

use graph::testing::{GraphSize, size, stored_concept_graph, stored_document};
use ocr::read_chapter;
use rag_ingestion::{DeleteSummary, Item, chapter_items, ingest_chapter};
use serde_json::json;

use crate::support::{
    self, RunRagIngest, StandInLlm, ThrowawayStores, assert_document_stored, points_in,
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
    let items_a = items_of(&support::intuition_chapter());
    let items_b = items_of(&support::in_depth_chapter());
    let (a, b) = (items_a.len() as u64, items_b.len() as u64);
    // Every item names the model that both chapters are about and one concept of its own chapter,
    // and says that the second is used for the first.
    let title_of_a = items_a[0].payload.doc_title.clone();
    let models = throwaway.models(StandInLlm::replying(move |input, _| {
        let own = if input.starts_with(&title_of_a) {
            "intuition about options"
        } else {
            "derivation of the model"
        };
        Ok(json!({
            "concepts": [
                { "name": "Black–Scholes model", "definition": "a model of option prices" },
                { "name": own, "definition": "a part of one chapter" },
            ],
            "relations": [{ "from": own, "type": "USED_FOR", "to": "Black–Scholes model" }],
        }))
    }));
    // What the graph holds when only the document of B, its items, its media, and the concepts
    // with their relations are left: the concepts and the two relations are three nodes and two
    // edges, and the media of both chapters is one more node.
    let size_with_b_and_concepts = GraphSize {
        nodes: b + 1 + 3 + 1,
        edges: b + (b - 1) + 2 * b + 2,
    };
    let first = ingest_chapter(
        support::chapter_at(&support::intuition_chapter()),
        &models,
        &stores,
    )
    .await
    .unwrap();
    let second = ingest_chapter(
        support::chapter_at(&support::in_depth_chapter()),
        &models,
        &stores,
    )
    .await
    .unwrap();
    assert_ne!(first.doc_id, second.doc_id);
    assert_eq!(second.points_in_collection, a + b);
    let before = stored_concept_graph(&stores.graph).await;
    assert_eq!(before.concepts.len(), 3);
    assert_eq!(before.mentions.len() as u64, 2 * a + 2 * b);
    assert_eq!(before.relations.len(), 2);

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
    assert_document_stored(&stores.graph, &items_b).await;
    assert_eq!(size(&stores.graph).await, size_with_b_and_concepts);
    assert_eq!(point_ids(&throwaway).await, ids_of(&items_b));
    let after = stored_concept_graph(&stores.graph).await;
    assert_eq!(
        after.concepts, before.concepts,
        "the concepts stay, with their ids"
    );
    let ids_of_b = ids_of(&items_b);
    let mentions_of_b: Vec<_> = before
        .mentions
        .iter()
        .filter(|mention| ids_of_b.contains(&mention.item))
        .cloned()
        .collect();
    assert_eq!(mentions_of_b.len() as u64, 2 * b);
    assert_eq!(
        after.mentions, mentions_of_b,
        "only the mentions of the deleted document go"
    );
    assert_eq!(after.relations, before.relations, "the relations stay");

    // A delete that stopped half way, after the points and before the nodes, is finished by
    // running it again.
    let removed = stores.items.delete_document(second.doc_id).await.unwrap();
    assert_eq!(removed, b);
    assert_eq!(size(&stores.graph).await, size_with_b_and_concepts);
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
    assert_eq!(
        size(&stores.graph).await,
        GraphSize { nodes: 4, edges: 2 },
        "the concepts, their relations and the media are left"
    );
    let left = stored_concept_graph(&stores.graph).await;
    assert_eq!(left.concepts, before.concepts);
    assert!(left.mentions.is_empty());
    assert_eq!(left.relations, before.relations);
    assert!(point_ids(&throwaway).await.is_empty());
}
