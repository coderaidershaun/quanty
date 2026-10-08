//! Runs a whole chapter through ingestion into a throwaway collection and graph, with an embedder
//! that makes up its vectors and a model that finds no concept, so nothing is billed.

use std::collections::{BTreeMap, BTreeSet};

use graph::{GraphStore, MediaNode};
use ocr::{PieceDetail, read_chapter};
use rag_core::{Category, DocumentLabels, MediaLabels, Tag};
use rag_ingestion::{
    ChapterFolder, Item, MediaChange, MediaRelabelled, RelabelError, TagChange, chapter_items,
    ingest_chapter, relabel_document_tags, relabel_media,
};
use serde_json::{Value, json};

use crate::support::{
    self, StandInLlm, ThrowawayStores, assert_graph_holds_only, assert_labelled, points_in,
};

fn tags(texts: &[&str]) -> Vec<Tag> {
    texts.iter().map(|text| text.parse().unwrap()).collect()
}

fn natenberg() -> MediaLabels {
    MediaLabels {
        category: Category::Book,
        authors: vec!["Sheldon Natenberg".to_owned()],
        tags: tags(&["options"]).into_iter().collect(),
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored ingest::"]
async fn ingest_fills_a_throwaway_collection_and_a_second_run_adds_nothing() {
    let throwaway = ThrowawayStores::new("ingest");
    let config = throwaway.config();
    let stores = throwaway.connect().await;
    let models = throwaway.models(StandInLlm::finding_nothing());
    let chapter = read_chapter(&support::sample_chapter()).unwrap();
    let items = chapter_items(&chapter);
    let media = natenberg();
    let sample_chapter = support::sample_chapter();
    let chapter_folder = ChapterFolder {
        folder: &sample_chapter,
        new_media: &media,
    };

    let summary = ingest_chapter(chapter_folder, &models, &stores)
        .await
        .unwrap();
    assert_eq!(summary.points_in_collection, items.len() as u64);

    let points = points_in(config).await;
    assert_eq!(points.len(), items.len());
    let stored_ids: BTreeSet<String> = points.iter().map(|(id, _)| id.clone()).collect();
    let item_ids: BTreeSet<String> = items.iter().map(|item| item.id.to_string()).collect();
    assert_eq!(stored_ids, item_ids);

    let stored_document = assert_graph_holds_only(&stores.graph, &items).await;
    let node_ids: BTreeSet<String> = stored_document
        .items
        .iter()
        .map(|item| item.id.clone())
        .collect();
    assert_eq!(node_ids, stored_ids, "a node has the id of its point");

    let formulas: Vec<&str> = chapter
        .pieces
        .iter()
        .filter(|piece| matches!(piece.detail, PieceDetail::Formula { .. }))
        .map(|piece| piece.content.as_str())
        .collect();
    let mut kinds = BTreeSet::new();
    for (_, payload) in &points {
        for field in [
            "doc_id",
            "doc_title",
            "page",
            "printed_page",
            "kind",
            "text",
        ] {
            assert!(
                !payload[field].is_null(),
                "a point has no {field}: {payload}"
            );
        }
        let kind = payload["kind"].as_str().unwrap();
        kinds.insert(kind.to_owned());
        match kind {
            "figure" => {
                let path = payload["image_path"]
                    .as_str()
                    .expect("a figure has its picture");
                assert!(path.ends_with("-figure.png"), "{path}");
                assert!(std::path::Path::new(path).is_file(), "{path}");
            }
            "formula" => {
                assert!(payload["image_path"].is_null());
                assert!(
                    formulas.contains(&payload["text"].as_str().unwrap()),
                    "a formula point holds the raw LaTeX"
                );
            }
            _ => assert!(payload["image_path"].is_null()),
        }
    }
    assert_eq!(
        kinds,
        ["chunk", "figure", "formula", "table"]
            .map(String::from)
            .into()
    );

    assert_label_and_cites_are_stored_only_where_the_item_has_them(&points, &items);
    assert_labelled(config, &stores.graph, &media, &[]).await;

    let received = models.embedder.received();
    assert_eq!(received.len(), items.len(), "one input for each item");
    let with_picture: Vec<_> = received
        .iter()
        .filter(|input| input.image.is_some())
        .collect();
    assert_eq!(with_picture.len(), 3);
    for input in with_picture {
        let picture = input.image.as_ref().unwrap();
        let figure = chapter
            .pieces
            .iter()
            .find(|piece| {
                piece
                    .figure_image
                    .as_ref()
                    .map(|image| image.path.canonicalize().unwrap())
                    == Some(picture.clone())
            })
            .expect("the picture belongs to a figure of the chapter");
        assert!(
            input.text.contains(&figure.content),
            "the explanation travels with the picture"
        );
    }

    let own_tags = TagChange {
        add: tags(&["  Options ", "options", "Volatility"]),
        remove: Vec::new(),
    };
    relabel_document_tags(summary.doc_id, &own_tags, &stores)
        .await
        .unwrap();
    let labelled = assert_labelled(config, &stores.graph, &media, &["options", "volatility"]).await;
    let labelled_ids: BTreeSet<String> = labelled.keys().cloned().collect();
    assert_eq!(
        labelled_ids, item_ids,
        "the points are the ones of the items"
    );

    // Other labels for a new media are not used, because the graph holds this media already.
    let other_media = MediaLabels {
        category: Category::Paper,
        authors: vec!["Someone Else".to_owned()],
        tags: BTreeSet::new(),
    };
    let chapter_folder = ChapterFolder {
        folder: &sample_chapter,
        new_media: &other_media,
    };
    let again = ingest_chapter(chapter_folder, &models, &stores)
        .await
        .unwrap();
    assert_eq!(again.points_in_collection, summary.points_in_collection);
    let points_after = points_in(config).await;
    let ids_after: BTreeSet<String> = points_after.into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids_after, stored_ids);
    assert_eq!(
        assert_graph_holds_only(&stores.graph, &items).await,
        stored_document,
        "a second run adds no node and no edge"
    );
    assert_eq!(
        assert_labelled(config, &stores.graph, &media, &["options", "volatility"]).await,
        labelled,
        "a second run keeps the stored media and the own tags that were set in between"
    );
    let received = models.embedder.received();
    assert_eq!(received.len(), 2 * items.len(), "labelling embeds nothing");
    assert_eq!(
        received[..items.len()],
        received[items.len()..],
        "labels do not change what is embedded"
    );

    relabel_document_tags(summary.doc_id, &own_tags, &stores)
        .await
        .unwrap();
    assert_eq!(
        assert_labelled(config, &stores.graph, &media, &["options", "volatility"]).await,
        labelled,
        "the same tags again change nothing"
    );

    let other_tags = TagChange {
        add: tags(&["Greeks"]),
        remove: tags(&["volatility"]),
    };
    relabel_document_tags(summary.doc_id, &other_tags, &stores)
        .await
        .unwrap();
    let changed = assert_labelled(config, &stores.graph, &media, &["greeks", "options"]).await;
    assert_eq!(changed.keys().cloned().collect::<BTreeSet<_>>(), stored_ids);
    assert_eq!(
        assert_graph_holds_only(&stores.graph, &items).await,
        stored_document,
        "other tags add no node and no edge"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored ingest::"]
async fn relabelling_a_media_rewrites_its_node_every_document_of_it_and_their_points() {
    let throwaway = ThrowawayStores::new("relabel-media");
    let config = throwaway.config();
    let stores = throwaway.connect().await;
    let models = throwaway.models(StandInLlm::finding_nothing());
    let notes = MediaLabels {
        category: Category::Book,
        authors: vec!["Quanty Team".to_owned()],
        tags: BTreeSet::new(),
    };
    let book = natenberg();
    let mut notes_ids = BTreeSet::new();
    for (folder, media) in [
        (support::intuition_chapter(), &notes),
        (support::in_depth_chapter(), &notes),
        (support::sample_chapter(), &book),
    ] {
        let chapter_folder = ChapterFolder {
            folder: &folder,
            new_media: media,
        };
        let summary = ingest_chapter(chapter_folder, &models, &stores)
            .await
            .unwrap();
        if media == &notes {
            notes_ids.insert(summary.doc_id);
        }
    }
    let first_note = *notes_ids.first().unwrap();
    let own_tags = TagChange {
        add: tags(&["intuition"]),
        remove: Vec::new(),
    };
    relabel_document_tags(first_note, &own_tags, &stores)
        .await
        .unwrap();
    let documents_before = stores.graph.documents().await.unwrap();
    let points_before: BTreeMap<String, Value> = points_in(config).await.into_iter().collect();

    let change = MediaChange {
        category: Some(Category::Paper),
        authors: Some(vec!["X".to_owned(), "Y".to_owned()]),
        tags: Some(tags(&["z"]).into_iter().collect()),
    };
    // Other capitals and spaces name the same media.
    let relabelled = relabel_media(" quanty sample NOTES ", &change, &stores)
        .await
        .unwrap();

    let new_labels = MediaLabels {
        category: Category::Paper,
        authors: vec!["X".to_owned(), "Y".to_owned()],
        tags: tags(&["z"]).into_iter().collect(),
    };
    assert_eq!(
        relabelled,
        MediaRelabelled {
            title: "Quanty Sample Notes".to_owned(),
            labels: new_labels.clone(),
            documents: 2,
        }
    );
    assert_eq!(
        stores.graph.media().await.unwrap(),
        vec![
            MediaNode {
                title: "Option Volatility and Pricing".to_owned(),
                labels: book,
            },
            MediaNode {
                title: "Quanty Sample Notes".to_owned(),
                labels: new_labels.clone(),
            },
        ]
    );
    let documents_after = stores.graph.documents().await.unwrap();
    assert_eq!(documents_after.len(), 3);
    for (before, after) in documents_before.iter().zip(&documents_after) {
        if notes_ids.contains(&before.id) {
            let mut expected = before.clone();
            expected
                .labels
                .take_media("Quanty Sample Notes", &new_labels);
            assert_eq!(after, &expected, "the own tags of a document stay");
        } else {
            assert_eq!(after, before, "a document of another media is not touched");
        }
    }
    let labels_of: BTreeMap<String, &DocumentLabels> = documents_after
        .iter()
        .map(|node| (node.id.to_string(), &node.labels))
        .collect();
    let points_after: BTreeMap<String, Value> = points_in(config).await.into_iter().collect();
    assert_eq!(
        points_after.keys().collect::<Vec<_>>(),
        points_before.keys().collect::<Vec<_>>()
    );
    for (id, after) in &points_after {
        let doc_id = after["doc_id"].as_str().unwrap();
        if notes_ids.iter().any(|note| note.to_string() == doc_id) {
            let stored: DocumentLabels = serde_json::from_value(after.clone()).unwrap();
            assert_eq!(
                &stored, labels_of[doc_id],
                "the point {id} carries its document's labels"
            );
        } else {
            assert_eq!(
                after, &points_before[id],
                "a point of another media is not touched"
            );
        }
    }

    let unknown = relabel_media("A Media Nobody Saved", &change, &stores)
        .await
        .unwrap_err();
    assert!(
        matches!(&unknown, RelabelError::UnknownMedia { title } if title == "A Media Nobody Saved"),
        "{unknown:?}"
    );
}

/// A point holds `label` and `cites` where its item has them and leaves the keys out everywhere
/// else, so a point stored before the two fields existed reads the same as a new one.
fn assert_label_and_cites_are_stored_only_where_the_item_has_them(
    points: &[(String, Value)],
    items: &[Item],
) {
    let (mut with_label, mut with_cites) = (0, 0);
    for (id, payload) in points {
        let item = items
            .iter()
            .find(|item| item.id.to_string() == *id)
            .expect("every point is an item of the chapter");
        match &item.payload.label {
            Some(label) => {
                assert_eq!(payload["label"], *label);
                with_label += 1;
            }
            None => assert!(payload.get("label").is_none(), "{payload}"),
        }
        if item.payload.cites.is_empty() {
            assert!(payload.get("cites").is_none(), "{payload}");
        } else {
            assert_eq!(payload["cites"], json!(item.payload.cites));
            with_cites += 1;
        }
    }
    assert!(with_label > 0, "the sample chapter has labelled items");
    assert!(with_cites > 0, "the sample chapter has chunks that cite");
}
