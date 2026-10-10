//! Writes edges whose nodes are missing and removes a media, on a throwaway graph, because only a
//! real FalkorDB shows which rows a statement drops and what a removal takes with it.

use graph::testing::{StoredMention, StoredRelation, stored_concept_graph};
use graph::{DocumentNode, GraphError, GraphStore, MediaNode, RelationKind};
use rag_core::{DocId, DocumentLabels, ItemKind, MediaLabels};

use crate::support::{concept, item, mention, relation, throwaway};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local FalkorDB from docker compose and bills nothing; run with: cargo test -p graph --test integration -- --ignored writes::"]
async fn a_mention_or_a_relation_whose_node_is_missing_is_reported_and_the_others_are_written() {
    let (_throwaway, graph) = throwaway("missing-nodes").await;
    let document = DocId::from_source_sha256("missing-nodes");
    let stored_item = item(document, ItemKind::Chunk, 0, 1);
    let missing_item = item(document, ItemKind::Chunk, 1, 1);
    let delta = concept("Delta hedging");
    let gamma = concept("Gamma");
    let missing_concept = concept("Vanna");
    graph
        .upsert_items(document, std::slice::from_ref(&stored_item))
        .await
        .unwrap();
    graph.upsert_concept(&delta).await.unwrap();
    graph.upsert_concept(&gamma).await.unwrap();

    let error = graph
        .add_mentions(&[
            mention(&missing_item, &delta, "the delta"),
            mention(&stored_item, &delta, "the delta"),
            mention(&stored_item, &missing_concept, "vanna"),
        ])
        .await
        .expect_err("two mentions name a node that is not stored");
    assert!(
        matches!(
            error,
            GraphError::MissingNodes {
                edges: "mentions",
                asked: 3,
                written: 1,
                ..
            }
        ),
        "{error:?}"
    );

    let error = graph
        .add_relations(&[
            relation(
                &delta,
                RelationKind::UsedFor,
                &missing_concept,
                &stored_item,
            ),
            relation(&gamma, RelationKind::PartOf, &delta, &stored_item),
        ])
        .await
        .expect_err("a relation names a concept that is not stored");
    assert!(
        matches!(
            error,
            GraphError::MissingNodes {
                edges: "relations",
                asked: 2,
                written: 1,
                ..
            }
        ),
        "{error:?}"
    );

    let written = stored_concept_graph(&graph).await;
    assert_eq!(
        written.mentions,
        vec![StoredMention {
            item: stored_item.id.to_string(),
            concept: "delta hedging".to_owned(),
            wording: "the delta".to_owned(),
        }]
    );
    assert_eq!(
        written.relations,
        vec![StoredRelation {
            from: "gamma".to_owned(),
            kind: "PART_OF".to_owned(),
            to: "delta hedging".to_owned(),
            item: stored_item.id.to_string(),
        }]
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local FalkorDB from docker compose and bills nothing; run with: cargo test -p graph --test integration -- --ignored writes::"]
async fn delete_media_removes_the_media_of_exactly_that_title_and_says_whether_one_was_there() {
    let (_throwaway, graph) = throwaway("delete-media").await;
    let notes = MediaNode {
        title: "Quanty Sample Notes".to_owned(),
        labels: MediaLabels::default(),
    };
    let volatility = MediaNode {
        title: "Option Volatility and Pricing".to_owned(),
        labels: MediaLabels::default(),
    };
    let document = DocumentNode {
        id: DocId::from_source_sha256("delete-media"),
        title: "Chapter 1".to_owned(),
        labels: DocumentLabels {
            media: Some(notes.title.clone()),
            ..DocumentLabels::default()
        },
    };
    graph.add_media(&notes).await.unwrap();
    graph.add_media(&volatility).await.unwrap();
    graph.upsert_document(&document).await.unwrap();

    assert!(!graph.delete_media("quanty sample notes").await.unwrap());
    assert_eq!(
        graph.media().await.unwrap(),
        vec![volatility.clone(), notes.clone()]
    );

    assert!(graph.delete_media(&notes.title).await.unwrap());
    assert_eq!(graph.media().await.unwrap(), vec![volatility]);
    assert_eq!(graph.documents().await.unwrap(), vec![document]);

    assert!(!graph.delete_media(&notes.title).await.unwrap());
}
