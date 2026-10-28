//! Reads that follow a concept out to the documents and items that mention it and to the concepts
//! it relates to, on a throwaway graph, because only a real FalkorDB shows that it accepts the
//! statements and orders their rows.

use std::collections::HashMap;

use graph::{
    ConceptAlias, ConceptDocument, ConceptItem, ConceptNode, Direction, DocumentNode, FalkorGraph,
    ItemNode, ItemPlace, PathDepth, Relation, RelationKind, SharedConcept,
};
use rag_core::{ConceptId, DocId, DocumentLabels, ItemId, ItemKind};

use crate::support::{concept, item, mention, relation, throwaway};

async fn write_document(graph: &FalkorGraph, id: DocId, title: &str, items: &[ItemNode]) {
    let node = DocumentNode {
        id,
        title: title.to_owned(),
        labels: DocumentLabels::default(),
    };
    graph.upsert_document(&node).await.unwrap();
    graph.upsert_items(id, items).await.unwrap();
}

fn concept_item(node: &ItemNode, document: DocId, wording: &str) -> ConceptItem {
    ConceptItem {
        item: node.id,
        document,
        page: node.page,
        printed_page: node.printed_page.clone(),
        kind: node.kind,
        wording: wording.to_owned(),
    }
}

fn shared(concept: &ConceptNode, mut mentions: Vec<(DocId, u64)>) -> SharedConcept {
    mentions.sort();
    SharedConcept {
        concept: concept.clone(),
        mentions,
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local FalkorDB from docker compose and bills nothing; run with: cargo test -p graph --test integration -- --ignored traversal::"]
async fn a_concept_comes_back_with_its_documents_aliases_items_and_places_and_documents_share_their_concepts()
 {
    let (_throwaway, graph) = throwaway("concept-reach").await;
    // The title that sorts first goes to the larger id, so a read in the order of the ids shows.
    let mut ids =
        ["a", "b", "c", "d"].map(|part| DocId::from_source_sha256(&format!("reach-{part}")));
    ids.sort();
    let [delta_notes, gamma_notes, beta_notes, alpha_notes] = ids;
    let [b0, b1, b2, b3] = [
        item(beta_notes, ItemKind::Chunk, 0, 1),
        item(beta_notes, ItemKind::Formula, 1, 1),
        item(beta_notes, ItemKind::Figure, 2, 2),
        ItemNode {
            printed_page: Some("xii".to_owned()),
            ..item(beta_notes, ItemKind::Chunk, 3, 3)
        },
    ];
    // Two chunks on page 2, so that only the id of the item tells them apart.
    let [a0, a1, a2, a3] = [
        item(alpha_notes, ItemKind::Chunk, 0, 1),
        item(alpha_notes, ItemKind::Chunk, 1, 2),
        item(alpha_notes, ItemKind::Chunk, 2, 2),
        item(alpha_notes, ItemKind::Table, 3, 2),
    ];
    let g0 = item(gamma_notes, ItemKind::Chunk, 0, 1);
    let d0 = item(delta_notes, ItemKind::Chunk, 0, 1);
    write_document(
        &graph,
        beta_notes,
        "Beta notes",
        &[b0.clone(), b1.clone(), b2.clone(), b3.clone()],
    )
    .await;
    write_document(
        &graph,
        alpha_notes,
        "Alpha notes",
        &[a0.clone(), a1.clone(), a2.clone(), a3.clone()],
    )
    .await;
    write_document(
        &graph,
        gamma_notes,
        "Gamma notes",
        std::slice::from_ref(&g0),
    )
    .await;
    write_document(
        &graph,
        delta_notes,
        "Delta notes",
        std::slice::from_ref(&d0),
    )
    .await;

    let [delta, gamma, rho, theta, vega] = ["Delta", "Gamma", "Rho", "Theta", "Vega"].map(concept);
    for written in [&delta, &gamma, &rho, &theta, &vega] {
        graph.upsert_concept(written).await.unwrap();
    }
    let delta_items = [&b0, &b1, &b2, &b3, &a0, &a1, &a2, &a3, &g0];
    let mut mentions: Vec<_> = delta_items
        .iter()
        .map(|node| mention(node, &delta, &format!("delta, said by {}", node.id)))
        .collect();
    mentions.extend([
        mention(&b3, &gamma, "gamma"),
        mention(&d0, &gamma, "gamma"),
        mention(&d0, &rho, "rho"),
        mention(&a0, &theta, "theta"),
    ]);
    graph.add_mentions(&mentions).await.unwrap();

    // The documents come by how many of their items mention the concept, the most first, and the
    // two with four come by title. The document with no mention is left out.
    let documents_of = |document, title: &str, mentions| ConceptDocument {
        document,
        title: title.to_owned(),
        mentions,
    };
    assert_eq!(
        graph.documents_for_concept(delta.id).await.unwrap(),
        vec![
            documents_of(alpha_notes, "Alpha notes", 4),
            documents_of(beta_notes, "Beta notes", 4),
            documents_of(gamma_notes, "Gamma notes", 1),
        ]
    );
    assert_eq!(
        graph.documents_for_concept(gamma.id).await.unwrap(),
        vec![
            documents_of(beta_notes, "Beta notes", 1),
            documents_of(delta_notes, "Delta notes", 1),
        ]
    );
    assert_eq!(graph.documents_for_concept(vega.id).await.unwrap(), vec![]);
    assert_eq!(
        graph
            .documents_for_concept(ConceptId::random())
            .await
            .unwrap(),
        vec![]
    );

    // The aliases come in the order they were added, which is not the order of the alphabet.
    for name in ["The delta", "Delta (option)"] {
        let alias = ConceptAlias {
            concept: delta.id,
            name: name.to_owned(),
            normalised_name: name.to_lowercase(),
        };
        graph.add_alias(&alias).await.unwrap();
    }
    assert_eq!(
        graph.aliases_of(delta.id).await.unwrap(),
        vec!["The delta".to_owned(), "Delta (option)".to_owned()]
    );
    assert_eq!(
        graph.aliases_of(gamma.id).await.unwrap(),
        Vec::<String>::new()
    );
    assert_eq!(
        graph.aliases_of(ConceptId::random()).await.unwrap(),
        Vec::<String>::new()
    );

    // By the title of the document, then the page, then the kind, then the id of the item.
    let said = |node: &ItemNode, document| {
        concept_item(node, document, &format!("delta, said by {}", node.id))
    };
    let (first_chunk, second_chunk) = if a1.id.to_string() < a2.id.to_string() {
        (&a1, &a2)
    } else {
        (&a2, &a1)
    };
    let all: Vec<ConceptItem> = vec![
        said(&a0, alpha_notes),
        said(first_chunk, alpha_notes),
        said(second_chunk, alpha_notes),
        said(&a3, alpha_notes),
        said(&b0, beta_notes),
        said(&b1, beta_notes),
        said(&b2, beta_notes),
        said(&b3, beta_notes),
        said(&g0, gamma_notes),
    ];
    let items_of = async |document, kind, offset, limit| {
        graph
            .items_mentioning(delta.id, document, kind, offset, limit)
            .await
            .unwrap()
    };
    assert_eq!(items_of(None, None, 0, 100).await, all);
    assert_eq!(items_of(None, None, 2, 3).await, all[2..5]);
    assert_eq!(items_of(None, None, 100, 3).await, vec![]);
    assert_eq!(items_of(Some(beta_notes), None, 0, 100).await, all[4..8]);
    let chunks = [&all[0], &all[1], &all[2], &all[4], &all[7], &all[8]];
    assert_eq!(
        items_of(None, Some(ItemKind::Chunk), 0, 100).await,
        chunks.map(Clone::clone)
    );
    assert_eq!(
        items_of(Some(beta_notes), Some(ItemKind::Chunk), 0, 100).await,
        vec![all[4].clone(), all[7].clone()]
    );
    assert_eq!(all[7].printed_page.as_deref(), Some("xii"));

    // An item that is not stored is left out.
    let unknown = ItemId::new(beta_notes, ItemKind::Code, 99);
    assert_eq!(
        graph.item_places(&[b0.id, a3.id, unknown]).await.unwrap(),
        HashMap::from([
            (
                b0.id,
                ItemPlace {
                    document: beta_notes,
                    page: 1
                }
            ),
            (
                a3.id,
                ItemPlace {
                    document: alpha_notes,
                    page: 2
                }
            ),
        ])
    );
    assert_eq!(graph.item_places(&[]).await.unwrap(), HashMap::new());

    // The most mentioned concept comes first, and the two with one mention come by name. Delta
    // notes mention no Delta, so it has no count there; Vega has no mention at all.
    assert_eq!(
        graph
            .concepts_shared(&[alpha_notes, beta_notes, delta_notes])
            .await
            .unwrap(),
        vec![
            shared(&delta, vec![(alpha_notes, 4), (beta_notes, 4)]),
            shared(&gamma, vec![(beta_notes, 1), (delta_notes, 1)]),
            shared(&rho, vec![(delta_notes, 1)]),
            shared(&theta, vec![(alpha_notes, 1)]),
        ]
    );
    assert_eq!(
        graph.concepts_shared(&[gamma_notes]).await.unwrap(),
        vec![shared(&delta, vec![(gamma_notes, 1)])]
    );
    assert_eq!(graph.concepts_shared(&[]).await.unwrap(), vec![]);

    assert_eq!(
        graph.top_concepts(3).await.unwrap(),
        vec![(delta.clone(), 9), (gamma.clone(), 2), (rho.clone(), 1)]
    );
    assert_eq!(
        graph.top_concepts(10).await.unwrap(),
        vec![(delta, 9), (gamma, 2), (rho, 1), (theta, 1)]
    );
}

fn by_edge(mut relations: Vec<Relation>) -> Vec<Relation> {
    relations.sort_by_key(|relation| {
        (
            relation.from.to_string(),
            relation.to.to_string(),
            relation.kind.as_str(),
        )
    });
    relations
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local FalkorDB from docker compose and bills nothing; run with: cargo test -p graph --test integration -- --ignored traversal::"]
async fn neighbours_follow_kind_and_direction_and_the_shortest_path_keeps_to_its_cap() {
    let (_throwaway, graph) = throwaway("concept-edges").await;
    let document = DocId::from_source_sha256("concept-edges");
    let statements: Vec<ItemNode> = (0u32..6)
        .map(|position| item(document, ItemKind::Chunk, position, 1))
        .collect();
    let [black_scholes, ito, calculus, hedging, volatility, lone] = [
        "Black–Scholes",
        "Itô's lemma",
        "Stochastic calculus",
        "Delta hedging",
        "Volatility",
        "Lone concept",
    ]
    .map(concept);
    for written in [
        &black_scholes,
        &ito,
        &calculus,
        &hedging,
        &volatility,
        &lone,
    ] {
        graph.upsert_concept(written).await.unwrap();
    }
    // Black–Scholes -> Itô, Calculus -> Itô, Itô -> Hedging, Black–Scholes -> Volatility and
    // Black–Scholes -> Hedging. The edge to Itô from Black–Scholes and the one from Calculus
    // reach the same concept from two sides.
    let derived = relation(
        &black_scholes,
        RelationKind::DerivedFrom,
        &ito,
        &statements[0],
    );
    let generalises = relation(&calculus, RelationKind::Generalises, &ito, &statements[1]);
    let used = relation(&ito, RelationKind::UsedFor, &hedging, &statements[2]);
    let assumes = relation(
        &black_scholes,
        RelationKind::Assumes,
        &volatility,
        &statements[3],
    );
    let direct = relation(
        &black_scholes,
        RelationKind::UsedFor,
        &hedging,
        &statements[4],
    );
    graph
        .add_relations(&[
            derived.clone(),
            generalises.clone(),
            used.clone(),
            assumes.clone(),
            direct.clone(),
        ])
        .await
        .unwrap();

    let around = async |concepts: &[&ConceptNode], kinds: &[RelationKind], direction, limit| {
        let ids: Vec<ConceptId> = concepts.iter().map(|concept| concept.id).collect();
        graph
            .neighbours(&ids, kinds, direction, limit)
            .await
            .unwrap()
    };

    // Out is the edges that leave Itô, In the edges that reach it, and Both either. Every edge
    // keeps the direction and the item it was written with.
    let out = around(&[&ito], &[], Direction::Out, 10).await;
    assert_eq!(out, vec![used.clone()]);
    let into = around(&[&ito], &[], Direction::In, 10).await;
    assert_eq!(into, by_edge(vec![derived.clone(), generalises.clone()]));
    let both = around(&[&ito], &[], Direction::Both, 10).await;
    assert_eq!(
        both,
        by_edge(vec![derived.clone(), generalises, used.clone()])
    );

    // Only the kinds that were asked for.
    let kinds = [RelationKind::DerivedFrom, RelationKind::UsedFor];
    assert_eq!(
        around(&[&ito], &kinds, Direction::Both, 10).await,
        by_edge(vec![derived.clone(), used.clone()])
    );

    // An edge between two of the asked concepts comes once, and so does an edge that touches only
    // one of them.
    assert_eq!(
        around(&[&black_scholes, &hedging], &[], Direction::Both, 10).await,
        by_edge(vec![
            derived.clone(),
            used.clone(),
            assumes.clone(),
            direct.clone()
        ])
    );
    assert_eq!(
        around(&[&black_scholes, &hedging], &[], Direction::Out, 10).await,
        by_edge(vec![derived, assumes, direct.clone()])
    );

    // The limit keeps the first edges of the order.
    let all = around(&[&black_scholes, &hedging], &[], Direction::Both, 10).await;
    assert_eq!(
        around(&[&black_scholes, &hedging], &[], Direction::Both, 3).await,
        all[..3]
    );
    assert_eq!(around(&[&lone], &[], Direction::Both, 10).await, vec![]);
    assert_eq!(
        graph
            .neighbours(&[], &[], Direction::Both, 10)
            .await
            .unwrap(),
        vec![]
    );

    // Calculus to Volatility crosses the edge to Itô with its arrow and the edge from
    // Black–Scholes against it. The way through Hedging is one edge longer.
    let path = |from: &ConceptNode, to: &ConceptNode, max_depth| {
        let max_depth = PathDepth::new(max_depth).expect("the test asks for 1 to 4 edges");
        graph.shortest_path(from.id, to.id, max_depth)
    };
    let chain = path(&calculus, &volatility, 4).await.unwrap();
    assert_eq!(
        chain,
        Some(vec![
            relation(&calculus, RelationKind::Generalises, &ito, &statements[1]),
            relation(
                &black_scholes,
                RelationKind::DerivedFrom,
                &ito,
                &statements[0]
            ),
            relation(
                &black_scholes,
                RelationKind::Assumes,
                &volatility,
                &statements[3]
            ),
        ])
    );
    assert_eq!(path(&calculus, &volatility, 3).await.unwrap(), chain);
    assert_eq!(path(&calculus, &volatility, 2).await.unwrap(), None);
    // The shortest way is the one edge, not the two edges through Itô.
    assert_eq!(
        path(&black_scholes, &hedging, 4).await.unwrap(),
        Some(vec![direct])
    );
    // The same chain is found from the other end, in the order from that end.
    assert_eq!(
        path(&volatility, &calculus, 4)
            .await
            .unwrap()
            .map(|edges| edges.len()),
        Some(3)
    );
    assert_eq!(path(&calculus, &lone, 4).await.unwrap(), None);
}
