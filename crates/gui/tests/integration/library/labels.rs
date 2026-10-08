//! Checks that the labels a person saves on a media and on a document reach both stores, with no
//! model asked.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use graph::{GraphStore, MediaNode};
use gui::backend::live::{LiveContext, RealServices, Services};
use gui::contract::{
    Category, DocId, Document, DocumentTagsEdit, FailureKind, MediaEdit, NewMedia,
};
use rag_core::{DocumentLabels, ItemFilter, MediaLabels, Tag};
use rag_ingestion::ingest_chapter;
use rag_ingestion::testing::{
    StandInEmbedder, StandInLlm, ThrowawayStores, chapter_at, first_axis,
};
use uuid::Uuid;

use super::{catalogue_of, document_tags_saved, media_edited, owned, saved};
use crate::support::{self, IN_DEPTH, INTUITION, sample_chapter};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p gui --test integration -- --ignored library::"]
async fn the_labels_a_person_saves_are_in_the_next_catalogue_and_on_every_point_and_no_model_is_made()
 {
    let stores = ThrowawayStores::new("library-relabel");
    let connected = stores.connect().await;
    let models = stores.models(StandInLlm::finding_nothing());
    let mut stored = Vec::new();
    for chapter in [INTUITION, IN_DEPTH] {
        let folder = sample_chapter(chapter);
        let summary = ingest_chapter(chapter_at(&folder), &models, &connected).await;
        stored.push(summary.expect("the chapter should be ingested"));
    }

    let answering = StandInLlm::finding_nothing();
    let embedders_made = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&embedders_made);
    let cx = support::context_with(stores, &answering, move || {
        counted.fetch_add(1, Ordering::SeqCst);
        StandInEmbedder::default()
    });

    let before = documents_of(&cx).await;
    let ids: Vec<DocId> = before.iter().map(|document| document.id).collect();
    let stored_ids: Vec<DocId> = stored.iter().map(|summary| summary.doc_id.into()).collect();
    assert_eq!(ids, stored_ids, "both chapters of the media are listed");
    for document in &before {
        assert_eq!(
            (
                document.authors.len(),
                document.media_tags.len(),
                document.tags.len()
            ),
            (0, 0, 0)
        );
    }

    let authors = ["Sheldon Natenberg", "Euan Sinclair"];
    let edit = MediaEdit {
        title: "quanty sample notes".to_owned(),
        category: Category::Paper,
        authors: owned(&authors),
        tags: owned(&["Options"]),
    };
    assert_eq!(media_edited(&cx, &edit).await, Ok(()));
    let opened = cx.stores().await.expect("the stores should open");
    let nodes = opened
        .graph
        .media()
        .await
        .expect("the media should be read");
    let node = MediaNode {
        title: "Quanty Sample Notes".to_owned(),
        labels: MediaLabels {
            category: rag_core::Category::Paper,
            authors: owned(&authors),
            tags: tag_set(&["options"]),
        },
    };
    assert_eq!(nodes, [node], "the edit lands on the media");
    let catalogue = catalogue_of(&cx).await.expect("the catalogue is read");
    let [media] = &catalogue.media[..] else {
        panic!("the library holds one media: {catalogue:#?}");
    };
    assert_eq!(
        (media.category, &media.authors, &media.tags),
        (Category::Paper, &owned(&authors), &owned(&["options"]))
    );
    assert_eq!(media.documents.len(), 2);
    for document in &media.documents {
        assert_eq!(document.authors, authors);
        assert_eq!(document.media_tags, ["options"]);
        assert_eq!(document.tags, Vec::<String>::new());
    }
    let mut wanted = DocumentLabels {
        media: Some("Quanty Sample Notes".to_owned()),
        category: Some(rag_core::Category::Paper),
        authors: owned(&authors),
        media_tags: tag_set(&["options"]),
        tags: BTreeSet::new(),
    };
    for summary in &stored {
        assert_every_point_carries(&cx, summary.doc_id, &wanted).await;
    }

    let own = DocumentTagsEdit::toward(&media.documents[0], &owned(&["volatility", "Greeks", " "]));
    assert_eq!(document_tags_saved(&cx, &own).await, Ok(()));
    let after_tags = documents_of(&cx).await;
    assert_eq!(after_tags[0].tags, ["greeks", "volatility"]);
    assert_eq!(
        after_tags[1].tags,
        Vec::<String>::new(),
        "own tags belong to one document"
    );
    for document in &after_tags {
        assert_eq!(
            (&document.authors, &document.media_tags),
            (&owned(&authors), &owned(&["options"])),
            "own tags leave the labels of the media alone"
        );
    }
    assert_every_point_carries(&cx, stored[1].doc_id, &wanted).await;
    wanted.tags = tag_set(&["greeks", "volatility"]);
    assert_every_point_carries(&cx, stored[0].doc_id, &wanted).await;

    assert_eq!(
        embedders_made.load(Ordering::SeqCst),
        0,
        "a change of labels makes no embedder"
    );
    assert_eq!(answering.calls(), 0, "a change of labels asks no model");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_save_of_labels_answers_once_when_the_stores_are_down() {
    let cx = LiveContext::new(support::closed_ports_config(), RealServices);
    let own = DocumentTagsEdit {
        doc: DocId(Uuid::from_u128(1)),
        add: owned(&["options"]),
        ..DocumentTagsEdit::default()
    };
    let media = NewMedia {
        title: "Dynamic Hedging".to_owned(),
        ..NewMedia::default()
    };
    let edit = MediaEdit {
        title: "Dynamic Hedging".to_owned(),
        ..MediaEdit::default()
    };

    let results = [
        document_tags_saved(&cx, &own).await,
        saved(&cx, &media).await,
        media_edited(&cx, &edit).await,
    ];

    for result in results {
        let failure = result.expect_err("no store answers, so no label can be written");
        assert!(
            matches!(
                failure.kind,
                FailureKind::FalkorDbDown | FailureKind::QdrantDown
            ),
            "{failure:?}"
        );
    }
}

/// In the order the catalogue lists them: chapters first, by number.
async fn documents_of<S: Services>(cx: &LiveContext<S>) -> Vec<Document> {
    let catalogue = catalogue_of(cx).await.expect("the catalogue is read");
    catalogue.documents().cloned().collect()
}

fn tag_set(texts: &[&str]) -> BTreeSet<Tag> {
    texts
        .iter()
        .map(|tag| tag.parse::<Tag>().expect("a tag is not blank"))
        .collect()
}

/// Every point of the document, not only the nearest ones, has these labels.
async fn assert_every_point_carries<S: Services>(
    cx: &LiveContext<S>,
    document: rag_core::DocId,
    wanted: &DocumentLabels,
) {
    let stores = cx.stores().await.expect("the stores should open");
    let filter = ItemFilter {
        kind: None,
        documents: Some(vec![document]),
    };
    let hits = stores.items.search(first_axis(), &filter, 1000).await;
    let hits = hits.expect("the points of the document should be read");
    let count = stores.items.count_document(document).await;
    let count = count.expect("the points of the document should be counted");
    assert!(count > 0, "the document has points");
    assert_eq!(hits.len() as u64, count, "every point was read");
    for hit in hits {
        assert_eq!(&hit.payload.document_labels, wanted, "point {}", hit.id);
    }
}
