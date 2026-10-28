//! Reads that answer for many concepts at once, on a throwaway graph, because only a real FalkorDB
//! shows that it accepts the statements and orders their rows.

use std::collections::HashMap;

use graph::{ConceptAlias, ConceptNode, DocumentNode, FalkorGraph, ItemNode, SharedDocument};
use rag_core::{ConceptId, DocId, DocumentLabels, ItemKind};

use crate::support::{concept, item, mention, throwaway};

/// Writes a document with one chunk for each list of concepts, and the mentions of those chunks.
async fn write_document(
    graph: &FalkorGraph,
    id: DocId,
    title: &str,
    chunks: &[&[&ConceptNode]],
) -> Vec<ItemNode> {
    let items: Vec<ItemNode> = (0u32..)
        .zip(chunks)
        .map(|(position, _)| item(id, ItemKind::Chunk, position, 1))
        .collect();
    graph
        .upsert_document(&DocumentNode {
            id,
            title: title.to_owned(),
            labels: DocumentLabels::default(),
        })
        .await
        .unwrap();
    graph.upsert_items(id, &items).await.unwrap();
    let mentions: Vec<_> = items
        .iter()
        .zip(chunks)
        .flat_map(|(item, concepts)| {
            concepts
                .iter()
                .map(|concept| mention(item, concept, &concept.name))
        })
        .collect();
    graph.add_mentions(&mentions).await.unwrap();
    items
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local FalkorDB from docker compose and bills nothing; run with: cargo test -p graph --test integration -- --ignored batches::"]
async fn concepts_their_aliases_and_their_documents_are_read_for_many_ids_at_once() {
    let (_throwaway, graph) = throwaway("batch-of-concepts").await;
    let document_a = DocId::from_source_sha256("batch-of-concepts-a");
    let document_b = DocId::from_source_sha256("batch-of-concepts-b");
    let [gamma, delta, theta, vega] = ["Gamma", "Delta", "Theta", "Vega"].map(concept);
    // Gamma is written before Delta and has two aliases, so a read in the order of the writes shows.
    // Theta is held and has no alias and no document. Vega is never written.
    for written in [&gamma, &delta, &theta] {
        graph.upsert_concept(written).await.unwrap();
    }
    write_document(&graph, document_a, "Document A", &[&[&delta], &[&delta]]).await;
    write_document(&graph, document_b, "Document B", &[&[&delta]]).await;
    // The aliases are not in the order of the alphabet, so a read that sorts them shows.
    for name in ["The gamma", "Gamma (greek)"] {
        let alias = ConceptAlias {
            concept: gamma.id,
            name: name.to_owned(),
            normalised_name: name.to_lowercase(),
        };
        graph.add_alias(&alias).await.unwrap();
    }
    let asked = [gamma.id, vega.id, delta.id, theta.id];

    // By normalised name, not in the order they were asked for. The id that is not held is left out.
    assert_eq!(
        graph.concepts_by_id(&asked).await.unwrap(),
        vec![delta.clone(), gamma.clone(), theta.clone()]
    );
    assert_eq!(
        graph.concept_aliases(&asked).await.unwrap(),
        HashMap::from([
            (
                gamma.id,
                vec!["The gamma".to_owned(), "Gamma (greek)".to_owned()]
            ),
            (delta.id, Vec::new()),
            (theta.id, Vec::new()),
        ])
    );
    // Delta has three items in two documents, and Theta is held and in none.
    assert_eq!(
        graph.document_counts(&asked).await.unwrap(),
        HashMap::from([(delta.id, 2), (gamma.id, 0), (theta.id, 0)])
    );

    let none: [ConceptId; 0] = [];
    assert_eq!(graph.concepts_by_id(&none).await.unwrap(), vec![]);
    assert_eq!(graph.concept_aliases(&none).await.unwrap(), HashMap::new());
    assert_eq!(graph.document_counts(&none).await.unwrap(), HashMap::new());
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local FalkorDB from docker compose and bills nothing; run with: cargo test -p graph --test integration -- --ignored batches::"]
async fn documents_that_share_concepts_come_by_how_many_they_share_then_by_title_then_by_id() {
    let (_throwaway, graph) = throwaway("documents-sharing").await;
    let [delta, gamma, theta, vega] = ["Delta", "Gamma", "Theta", "Vega"].map(concept);
    for written in [&delta, &gamma, &theta, &vega] {
        graph.upsert_concept(written).await.unwrap();
    }
    let id_of = |name: &str| DocId::from_source_sha256(&format!("documents-sharing-{name}"));
    let own = id_of("own");
    // Two documents with one title, so that the id decides which comes first.
    let mut twins = [id_of("twin-a"), id_of("twin-b")];
    twins.sort();
    // The document that the question is about shares all three, but is left out of the answer.
    write_document(&graph, own, "Own", &[&[&delta, &gamma, &theta]]).await;
    // Delta is mentioned by two of its items, and still counts once.
    write_document(
        &graph,
        id_of("wide"),
        "Wide",
        &[&[&delta, &gamma], &[&delta, &theta]],
    )
    .await;
    // The twins are written in the order of the ids reversed, and are listed after "Alpha".
    write_document(&graph, twins[1], "Twin", &[&[&delta, &gamma]]).await;
    write_document(&graph, twins[0], "Twin", &[&[&gamma, &theta]]).await;
    write_document(&graph, id_of("alpha"), "Alpha", &[&[&delta, &theta]]).await;
    write_document(&graph, id_of("zed"), "Zed", &[&[&theta]]).await;
    // Vega is not among the concepts asked for.
    write_document(&graph, id_of("other"), "Other", &[&[&vega]]).await;
    let shared = |document, title: &str, shared| SharedDocument {
        document,
        title: title.to_owned(),
        shared,
    };

    assert_eq!(
        graph
            .documents_sharing_concepts(&[delta.id, gamma.id, theta.id], own)
            .await
            .unwrap(),
        vec![
            shared(id_of("wide"), "Wide", 3),
            shared(id_of("alpha"), "Alpha", 2),
            shared(twins[0], "Twin", 2),
            shared(twins[1], "Twin", 2),
            shared(id_of("zed"), "Zed", 1),
        ]
    );
    assert_eq!(
        graph.documents_sharing_concepts(&[], own).await.unwrap(),
        vec![]
    );
}
