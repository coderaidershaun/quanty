//! Reads a throwaway graph that holds two documents, four concepts, their mentions and two
//! relations, because only a real FalkorDB shows that the three reads give each item and concept
//! once, in the order and up to the limit that they promise.

use graph::{
    ConceptNode, DocumentNode, GraphStore, ItemMentions, ItemNode, Mention, Relation, RelationKind,
};
use rag_core::{ConceptId, DocId, ItemId, ItemKind};
use rag_ingestion::testing::ThrowawayStores;

fn concept(name: &str) -> ConceptNode {
    ConceptNode {
        id: ConceptId::random(),
        name: name.to_owned(),
        normalised_name: name.to_lowercase(),
        definition: format!("{name} in one line"),
    }
}

fn item(document: DocId, position: u32) -> ItemNode {
    ItemNode {
        id: ItemId::new(document, ItemKind::Chunk, position),
        kind: ItemKind::Chunk,
        page: 1,
        printed_page: None,
    }
}

fn mention(item: &ItemNode, concept: &ConceptNode) -> Mention {
    Mention {
        item: item.id,
        concept: concept.id,
        wording: concept.name.clone(),
    }
}

fn relation(from: &ConceptNode, kind: RelationKind, to: &ConceptNode, item: &ItemNode) -> Relation {
    Relation {
        from: from.id,
        to: to.id,
        kind,
        item: item.id,
    }
}

/// The concepts of each item are in no fixed order, so sort them: only the order of the items is
/// a promise.
fn with_sorted_concepts(mentions: Vec<ItemMentions>) -> Vec<(ItemId, Vec<ConceptId>)> {
    mentions
        .into_iter()
        .map(|mentioned| {
            let mut concepts = mentioned.concepts;
            concepts.sort();
            (mentioned.item, concepts)
        })
        .collect()
}

fn sorted(mut concepts: Vec<ConceptId>) -> Vec<ConceptId> {
    concepts.sort();
    concepts
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-retrieval --test integration -- --ignored graph_reads::"]
async fn the_three_reads_follow_mentions_and_one_relation_hop_on_a_throwaway_graph() {
    let throwaway = ThrowawayStores::new("graph-reads");
    let graph = throwaway.connect().await.graph;
    let document_a = DocId::from_source_sha256("graph-reads-a");
    let document_b = DocId::from_source_sha256("graph-reads-b");
    let (i1, i2) = (item(document_a, 0), item(document_a, 1));
    let (i3, i4) = (item(document_b, 0), item(document_b, 1));
    let c1 = concept("Black–Scholes model");
    let c2 = concept("Itô's lemma");
    let c3 = concept("Stochastic calculus");
    let c4 = concept("Delta hedging");
    let documents = [
        (document_a, "Document A", [i1.clone(), i2.clone()]),
        (document_b, "Document B", [i3.clone(), i4.clone()]),
    ];
    for (id, title, items) in &documents {
        let document = DocumentNode {
            id: *id,
            title: (*title).to_owned(),
        };
        graph.upsert_document(&document).await.unwrap();
        graph.upsert_items(*id, items).await.unwrap();
    }
    for concept in [&c1, &c2, &c3, &c4] {
        graph.upsert_concept(concept).await.unwrap();
    }
    let mentions = [
        mention(&i1, &c1),
        mention(&i2, &c1),
        mention(&i2, &c2),
        mention(&i3, &c2),
        mention(&i4, &c3),
    ];
    graph.add_mentions(&mentions).await.unwrap();
    let relations = [
        relation(&c4, RelationKind::UsedFor, &c1, &i1),
        relation(&c2, RelationKind::PartOf, &c3, &i3),
    ];
    graph.add_relations(&relations).await.unwrap();

    // The concept that two of the items mention comes first, and each concept comes once.
    let of_items = graph.concepts_for_items(&[i1.id, i2.id]).await.unwrap();
    assert_eq!(of_items, vec![c1.clone(), c2.clone()]);

    // The item that mentions the most of the concepts comes first, then the others by id, and
    // each item comes once with only the concepts that were asked about.
    let mut others = [(i1.id, vec![c1.id]), (i3.id, vec![c2.id])];
    others.sort_by_key(|(id, _)| id.to_string());
    let mut expected = vec![(i2.id, sorted(vec![c1.id, c2.id]))];
    expected.extend(others);
    let of_concepts = graph.items_for_concepts(&[c1.id, c2.id], 10).await.unwrap();
    assert_eq!(with_sorted_concepts(of_concepts), expected);
    let limited = graph.items_for_concepts(&[c1.id, c2.id], 1).await.unwrap();
    assert_eq!(
        with_sorted_concepts(limited),
        vec![(i2.id, sorted(vec![c1.id, c2.id]))]
    );

    // One hop in either direction, never a concept that was given, each concept once.
    let to_c1 = graph.related_concepts(&[c1.id]).await.unwrap();
    assert_eq!(to_c1, vec![c4.clone()], "the edge points at the concept");
    let from_c2 = graph.related_concepts(&[c2.id]).await.unwrap();
    assert_eq!(from_c2, vec![c3.clone()], "the edge points away from it");
    let both_ends = graph.related_concepts(&[c2.id, c3.id]).await.unwrap();
    assert_eq!(both_ends, Vec::<ConceptNode>::new());
    let two_hops = graph.related_concepts(&[c1.id, c2.id]).await.unwrap();
    assert_eq!(two_hops, vec![c4, c3], "by normalised name, each once");

    // Nothing asked about means no call and no row.
    assert_eq!(graph.concepts_for_items(&[]).await.unwrap(), vec![]);
    assert_eq!(graph.items_for_concepts(&[], 10).await.unwrap(), vec![]);
    assert_eq!(graph.related_concepts(&[]).await.unwrap(), vec![]);
}
