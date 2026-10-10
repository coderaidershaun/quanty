//! Checks that a delete reaches both stores and the content folder: the document or the media is
//! gone from the next catalogue, from Qdrant, from the graph and from disk, and what else is
//! stored stays whole.
//!
//! Every context here has the temporary content folder of its own throwaway stores, and every
//! chapter is copied into it before it is ingested, so a delete can never reach the committed
//! samples or the real library.

use std::path::{Path, PathBuf};

use graph::GraphStore;
use gui::backend::live::{LiveContext, Services};
use gui::contract::{DocId, FailureKind};
use rag_ingestion::ingest_chapter;
use rag_ingestion::testing::{StandInEmbedder, StandInLlm, ThrowawayStores, chapter_at};

use super::{catalogue_of, document_deleted, media_deleted};
use crate::support::{self, IN_DEPTH, INTUITION, SAMPLE_PAGES, sample_chapter};

const NOTES: &str = "Quanty Sample Notes";
const BOOK: &str = "Option Volatility and Pricing";

/// A chapter that is stored: the copy under the content folder that it was ingested from, and the
/// id it was stored under.
struct Stored {
    folder: PathBuf,
    doc: DocId,
}

/// Copies the committed chapter under the content folder, to the same media folder and chapter
/// folder, and ingests the copy.
async fn ingest_a_copy(
    chapter: &str,
    content: &Path,
    models: &rag_ingestion::Models<StandInEmbedder, StandInLlm>,
    connected: &rag_ingestion::Stores<graph::FalkorGraph>,
) -> Stored {
    let copy = content.join(chapter);
    support::copy_folder(&sample_chapter(chapter), &copy);
    let folder = std::fs::canonicalize(copy).expect("the copy should exist");
    let summary = ingest_chapter(chapter_at(&folder), models, connected)
        .await
        .expect("the chapter should be ingested");
    Stored {
        folder,
        doc: summary.doc_id.into(),
    }
}

async fn points_of<S: Services>(cx: &LiveContext<S>, doc: DocId) -> u64 {
    let stores = cx.stores().await.expect("the stores should open");
    stores
        .items
        .count_document(doc.into())
        .await
        .expect("the points of the document should be counted")
}

async fn has_a_record<S: Services>(cx: &LiveContext<S>, doc: DocId) -> bool {
    let stores = cx.stores().await.expect("the stores should open");
    let records = stores
        .graph
        .document_records()
        .await
        .expect("the records should be read");
    let doc: rag_core::DocId = doc.into();
    records.iter().any(|record| record.node.id == doc)
}

/// The committed samples are never a delete's to touch.
fn the_samples_are_whole() {
    for chapter in [INTUITION, IN_DEPTH, SAMPLE_PAGES] {
        assert!(
            sample_chapter(chapter).is_dir(),
            "the committed chapter {chapter} is still there"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p gui --test integration -- --ignored library::"]
async fn a_deleted_document_is_gone_from_the_catalogue_from_both_stores_and_from_the_content_folder()
 {
    let stores = ThrowawayStores::new("library-delete-document");
    let content = stores.config().content_folder.clone();
    let connected = stores.connect().await;
    let models = stores.models(StandInLlm::finding_nothing());
    let first = ingest_a_copy(INTUITION, &content, &models, &connected).await;
    let second = ingest_a_copy(IN_DEPTH, &content, &models, &connected).await;
    let cx = support::context_with(
        stores,
        &StandInLlm::finding_nothing(),
        StandInEmbedder::default,
    );
    let second_points = points_of(&cx, second.doc).await;
    assert!(points_of(&cx, first.doc).await > 0);
    assert!(second_points > 0);

    assert_eq!(document_deleted(&cx, first.doc).await, Ok(()));

    let catalogue = catalogue_of(&cx).await.expect("the catalogue is read");
    let listed: Vec<DocId> = catalogue.documents().map(|document| document.id).collect();
    assert_eq!(listed, [second.doc], "only the other chapter is listed");
    assert_eq!(points_of(&cx, first.doc).await, 0, "its points are gone");
    assert_eq!(
        points_of(&cx, second.doc).await,
        second_points,
        "the other chapter keeps its points"
    );
    assert!(!has_a_record(&cx, first.doc).await, "its nodes are gone");
    assert!(has_a_record(&cx, second.doc).await);
    assert!(!first.folder.exists(), "its converted folder is gone");
    assert!(second.folder.is_dir());
    the_samples_are_whole();
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p gui --test integration -- --ignored library::"]
async fn a_deleted_media_is_gone_with_every_document_of_it_and_another_media_is_whole() {
    let stores = ThrowawayStores::new("library-delete-media");
    let content = stores.config().content_folder.clone();
    let connected = stores.connect().await;
    let models = stores.models(StandInLlm::finding_nothing());
    let notes = [
        ingest_a_copy(INTUITION, &content, &models, &connected).await,
        ingest_a_copy(IN_DEPTH, &content, &models, &connected).await,
    ];
    let book = ingest_a_copy(SAMPLE_PAGES, &content, &models, &connected).await;
    let cx = support::context_with(
        stores,
        &StandInLlm::finding_nothing(),
        StandInEmbedder::default,
    );
    let book_points = points_of(&cx, book.doc).await;
    assert!(book_points > 0);

    assert_eq!(media_deleted(&cx, NOTES).await, Ok(()));

    let catalogue = catalogue_of(&cx).await.expect("the catalogue is read");
    let titles: Vec<Option<&str>> = catalogue
        .media
        .iter()
        .map(|media| media.title.as_deref())
        .collect();
    assert_eq!(titles, [Some(BOOK)], "only the other media is listed");
    let listed: Vec<DocId> = catalogue.documents().map(|document| document.id).collect();
    assert_eq!(listed, [book.doc]);
    for stored in &notes {
        assert_eq!(points_of(&cx, stored.doc).await, 0, "its points are gone");
        assert!(!has_a_record(&cx, stored.doc).await, "its nodes are gone");
        assert!(!stored.folder.exists(), "its converted folder is gone");
    }
    let media_folder = content.join("quanty-sample-notes");
    assert!(!media_folder.exists(), "the folder of the media is gone");
    assert_eq!(
        points_of(&cx, book.doc).await,
        book_points,
        "the other media keeps its points"
    );
    assert!(has_a_record(&cx, book.doc).await);
    assert!(book.folder.is_dir());
    the_samples_are_whole();
}

#[tokio::test(flavor = "multi_thread")]
async fn each_delete_answers_once_with_a_failure_when_the_stores_are_down() {
    let content = tempfile::tempdir().expect("a temporary folder should be made");
    let cx = support::closed(content.path());
    let doc = DocId(uuid::Uuid::from_u128(1));

    let results = [
        document_deleted(&cx, doc).await,
        media_deleted(&cx, NOTES).await,
    ];

    for result in results {
        let failure = result.expect_err("no store answers, so nothing can be deleted");
        assert!(
            matches!(
                failure.kind,
                FailureKind::FalkorDbDown | FailureKind::QdrantDown
            ),
            "{failure:?}"
        );
    }
}
