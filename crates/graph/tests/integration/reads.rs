//! Reads what the Library, the concept graph and the source page ask of the store, on a throwaway
//! graph, because only a real FalkorDB shows that it accepts the statements and orders their rows.

use std::collections::BTreeSet;
use std::iter::repeat_n;
use std::path::Path;

use graph::testing::ThrowawayGraph;
use graph::{
    ConceptNode, DocumentNode, DocumentRecord, FalkorGraph, GraphStore, ItemNode, ItemsByKind,
    MediaNode, Mention, Relation, RelationKind,
};
use rag_core::{
    Category, ConceptId, Config, DocId, DocumentLabels, ItemId, ItemKind, MediaLabels, Tag,
};

const THROWAWAY_PREFIX: &str = "test-graph-";

/// A graph that no one else uses, and a connection to it. The graph is removed when the first
/// value is dropped, so keep it for the whole test.
async fn throwaway(test_name: &str) -> (ThrowawayGraph, FalkorGraph) {
    let settings = Config::load().expect("the settings should load");
    let throwaway = ThrowawayGraph::new(&settings, test_name);
    let config = Config {
        falkordb_graph: throwaway.name().to_owned(),
        ..settings
    };
    // The name is copied into the config by hand. Without this check, a copy that is missing would
    // leave the real graph in the config, and the test would write to it.
    assert!(config.falkordb_graph.starts_with(THROWAWAY_PREFIX));
    let graph = FalkorGraph::connect(&config)
        .await
        .expect("FalkorDB should answer");
    (throwaway, graph)
}

fn tag(text: &str) -> Tag {
    text.parse().expect("a tag is not empty")
}

fn concept(name: &str) -> ConceptNode {
    ConceptNode {
        id: ConceptId::random(),
        name: name.to_owned(),
        normalised_name: name.to_lowercase(),
        definition: format!("{name} in one line"),
    }
}

fn item(document: DocId, kind: ItemKind, position: u32, page: u32) -> ItemNode {
    ItemNode {
        id: ItemId::new(document, kind, position),
        kind,
        page,
        printed_page: None,
    }
}

fn mention(item: &ItemNode, concept: &ConceptNode, wording: &str) -> Mention {
    Mention {
        item: item.id,
        concept: concept.id,
        wording: wording.to_owned(),
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

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local FalkorDB from docker compose and bills nothing; run with: cargo test -p graph --test integration -- --ignored reads::"]
async fn document_records_come_back_with_their_labels_mark_folder_and_items_by_kind() {
    let (_throwaway, graph) = throwaway("document-records").await;
    // The first title belongs to the larger id and to the document that is written last, so a read
    // in the order of the ids, or in the order of the writes, would give the other order.
    let mut ids = [
        DocId::from_source_sha256("document-records-a"),
        DocId::from_source_sha256("document-records-b"),
    ];
    ids.sort();
    let [id_b, id_a] = ids;
    let node_a = DocumentNode {
        id: id_a,
        title: "Document A".to_owned(),
        labels: DocumentLabels {
            media: Some("Options and Volatility".to_owned()),
            category: Some(Category::Paper),
            // Not in the order of the alphabet, so a read that sorts the authors shows.
            authors: vec!["B Author".to_owned(), "A Author".to_owned()],
            media_tags: BTreeSet::from([tag("options")]),
            tags: BTreeSet::from([tag("volatility")]),
        },
    };
    let node_b = DocumentNode {
        id: id_b,
        title: "Document B".to_owned(),
        labels: DocumentLabels::default(),
    };
    // A different number of items of each kind, so that a kind counted as another one shows.
    let kinds = repeat_n(ItemKind::Chunk, 4)
        .chain(repeat_n(ItemKind::Formula, 3))
        .chain(repeat_n(ItemKind::Figure, 2))
        .chain(repeat_n(ItemKind::Table, 1));
    let items: Vec<ItemNode> = (0u32..)
        .zip(kinds)
        .map(|(position, kind)| item(id_a, kind, position, 1))
        .collect();
    let folder = Path::new("/chapters/options-and-volatility/chapter-1");

    graph.upsert_document(&node_b).await.unwrap();
    graph.upsert_document(&node_a).await.unwrap();
    graph.upsert_items(id_a, &items).await.unwrap();
    graph.set_ingested_items(id_a, Some(10)).await.unwrap();
    graph.set_chapter_folder(id_a, folder).await.unwrap();

    let expected = vec![
        DocumentRecord {
            node: node_a,
            ingested_items: Some(10),
            chapter_folder: Some(folder.to_path_buf()),
            items: ItemsByKind {
                chunks: 4,
                formulas: 3,
                figures: 2,
                tables: 1,
            },
        },
        DocumentRecord {
            node: node_b,
            ingested_items: None,
            chapter_folder: None,
            items: ItemsByKind::default(),
        },
    ];
    assert_eq!(graph.document_records().await.unwrap(), expected);

    // A folder for an id that is not stored is not a document: nothing is created.
    let unknown = DocId::from_source_sha256("document-records-unknown");
    graph
        .set_chapter_folder(unknown, Path::new("/chapters/nowhere"))
        .await
        .unwrap();
    assert_eq!(graph.document_records().await.unwrap(), expected);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local FalkorDB from docker compose and bills nothing; run with: cargo test -p graph --test integration -- --ignored reads::"]
async fn the_edges_among_what_was_asked_and_the_concepts_of_a_page_come_back_and_nothing_else() {
    let (_throwaway, graph) = throwaway("edges-and-page").await;
    let document_a = DocId::from_source_sha256("edges-and-page-a");
    let document_b = DocId::from_source_sha256("edges-and-page-b");
    let i1 = item(document_a, ItemKind::Chunk, 0, 1);
    let i2 = item(document_a, ItemKind::Formula, 1, 1);
    let i3 = item(document_a, ItemKind::Figure, 2, 2);
    let i4 = item(document_b, ItemKind::Chunk, 0, 1);
    // The name of the first concept sorts after the name of the second.
    let c1 = concept("The Black–Scholes model");
    let c2 = concept("Itô's lemma");
    let c3 = concept("Stochastic calculus");
    let c4 = concept("Delta hedging");

    for (id, title, items) in [
        (
            document_a,
            "Document A",
            vec![i1.clone(), i2.clone(), i3.clone()],
        ),
        (document_b, "Document B", vec![i4.clone()]),
    ] {
        let document = DocumentNode {
            id,
            title: title.to_owned(),
            labels: DocumentLabels::default(),
        };
        graph.upsert_document(&document).await.unwrap();
        graph.upsert_items(id, &items).await.unwrap();
    }
    for concept in [&c1, &c2, &c3, &c4] {
        graph.upsert_concept(concept).await.unwrap();
    }
    graph
        .add_mentions(&[
            mention(&i1, &c1, "the model"),
            mention(&i2, &c1, "the Black–Scholes formula"),
            mention(&i2, &c2, "Itô"),
            mention(&i3, &c3, "stochastic calculus"),
            mention(&i4, &c4, "hedging the delta"),
        ])
        .await
        .unwrap();
    let derived_from = relation(&c1, RelationKind::DerivedFrom, &c2, &i2);
    let part_of = relation(&c3, RelationKind::PartOf, &c1, &i3);
    graph
        .add_relations(&[
            derived_from.clone(),
            part_of.clone(),
            relation(&c2, RelationKind::UsedFor, &c4, &i4),
        ])
        .await
        .unwrap();

    // Both ends among the asked concepts, with the direction and the item as they were written,
    // by the id of the end the arrow leaves. The edge to a concept that was not asked is left out.
    let mut relations = vec![derived_from.clone(), part_of];
    relations.sort_by_key(|relation| relation.from.to_string());
    let asked = [c1.id, c2.id, c3.id];
    assert_eq!(graph.relations_among(&asked).await.unwrap(), relations);
    // The edge that leaves a concept that was not asked is left out too.
    assert_eq!(
        graph.relations_among(&[c2.id, c1.id]).await.unwrap(),
        vec![derived_from]
    );

    // An item and a concept that were both asked for, by item id and then by concept id. The
    // mention of a concept that was not asked, and of an item that was not asked, are left out.
    let mut mentions = vec![
        mention(&i2, &c1, "the Black–Scholes formula"),
        mention(&i3, &c3, "stochastic calculus"),
    ];
    mentions.sort_by_key(|mention| (mention.item.to_string(), mention.concept.to_string()));
    assert_eq!(
        graph
            .mentions_between(&[i2.id, i3.id], &[c1.id, c3.id])
            .await
            .unwrap(),
        mentions
    );

    // The concept that more items of the page mention comes first, before the one whose name sorts
    // first. Nothing of the other page, the other document or a page that has no item comes back.
    assert_eq!(
        graph.concepts_on_page(document_a, 1).await.unwrap(),
        vec![c1.clone(), c2.clone()]
    );
    assert_eq!(graph.concepts_on_page(document_a, 3).await.unwrap(), vec![]);

    assert_eq!(graph.relations_among(&[]).await.unwrap(), vec![]);
    assert_eq!(graph.mentions_between(&[], &[c1.id]).await.unwrap(), vec![]);
    assert_eq!(graph.mentions_between(&[i2.id], &[]).await.unwrap(), vec![]);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local FalkorDB from docker compose and bills nothing; run with: cargo test -p graph --test integration -- --ignored reads::"]
async fn media_come_back_by_title_and_only_an_update_changes_a_stored_one() {
    let (_throwaway, graph) = throwaway("media").await;
    let paper = MediaNode {
        title: "Hawkes Processes".to_owned(),
        labels: MediaLabels {
            category: Category::Paper,
            // Not in the order of the alphabet, so a read that sorts the authors shows.
            authors: vec!["B Author".to_owned(), "A Author".to_owned()],
            tags: BTreeSet::from([tag("hawkes")]),
        },
    };
    let book = MediaNode {
        title: "A Book".to_owned(),
        labels: MediaLabels::default(),
    };
    graph.add_media(&paper).await.unwrap();
    graph.add_media(&book).await.unwrap();

    // A second add of a stored title keeps the labels it has.
    let other_labels = MediaNode {
        title: paper.title.clone(),
        labels: MediaLabels {
            category: Category::Other,
            authors: vec!["Someone Else".to_owned()],
            tags: BTreeSet::new(),
        },
    };
    graph.add_media(&other_labels).await.unwrap();
    assert_eq!(graph.media().await.unwrap(), vec![book.clone(), paper]);

    graph.update_media(&other_labels).await.unwrap();
    assert_eq!(graph.media().await.unwrap(), vec![book, other_labels]);
}
