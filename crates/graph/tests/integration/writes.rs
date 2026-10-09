//! Writes edges whose nodes are missing, on a throwaway graph, because only a real FalkorDB shows
//! which rows a statement drops.

use graph::testing::{StoredMention, StoredRelation, stored_concept_graph};
use graph::{GraphError, GraphStore, RelationKind};
use rag_core::{DocId, ItemKind};

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
