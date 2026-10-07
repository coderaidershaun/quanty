//! Checks that the live backend lists the stored documents, each with the chapter that is on
//! disk for it.

use std::path::{Path, PathBuf};

use graph::GraphStore;
use gui::backend::Reply;
use gui::backend::live::{LiveContext, RealServices, Services, library};
use gui::contract::{
    Book, Catalogue, ChapterLabel, Document, Event, Failure, ItemCounts, RequestId,
};
use rag_ingestion::{IngestSummary, ingest_chapter};

use crate::support;

const REQUEST: RequestId = RequestId(7);
const INTUITION: &str = "quanty-sample-notes/chapter-1";
const IN_DEPTH: &str = "quanty-sample-notes/chapter-2";

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p gui --test integration -- --ignored library::"]
async fn the_catalogue_joins_stored_documents_with_their_chapter_folders() {
    let cx = support::context("library-join");
    let stores = cx.stores().await.expect("the stores should open");
    let models = cx.models().expect("the models should be made");
    let content = cx.config().content_folder.clone();
    // The first chapter is ingested where it is committed, and the graph keeps that folder.
    let intuition = sample_chapter(INTUITION);
    let first = ingest_chapter(&intuition, &models, &stores).await;
    let first = first.expect("the first chapter should be ingested");
    // The second chapter is ingested from a copy under the content folder, and the graph keeps a
    // folder that is not there, so the chapter must be found again by its id.
    let copy = content.join(IN_DEPTH);
    copy_folder(&sample_chapter(IN_DEPTH), &copy);
    let second = ingest_chapter(&copy, &models, &stores).await;
    let second = second.expect("the second chapter should be ingested");
    let graph = &stores.graph;
    graph
        .set_chapter_folder(first.doc_id, &intuition)
        .await
        .expect("the first folder should be stored");
    graph
        .set_chapter_folder(second.doc_id, &content.join("moved-away"))
        .await
        .expect("the second folder should be stored");

    let result = catalogue_of(&cx).await;

    let expected = Catalogue {
        books: vec![Book {
            title: Some("Quanty Sample Notes".to_owned()),
            chapters: vec![
                shown(&first, 1, "Options Pricing Intuition", &intuition),
                shown(&second, 2, "Black Scholes In Depth", &copy),
            ],
        }],
    };
    assert_eq!(result, Ok(expected));
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p gui --test integration -- --ignored library::"]
async fn new_stores_give_an_empty_catalogue() {
    let cx = support::context("library-empty");

    let result = catalogue_of(&cx).await;

    assert_eq!(result, Ok(Catalogue { books: Vec::new() }));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_catalogue_answers_once_when_the_stores_are_down() {
    let cx = LiveContext::new(support::closed_ports_config(), RealServices);

    let result = catalogue_of(&cx).await;

    result.expect_err("no store answers, so the catalogue cannot be read");
}

/// Loads the catalogue and returns what its one event carries, after checking that exactly one
/// event was sent, with the id of the request. The `Send` bound is a proof at compile time that
/// the window can run the load on any thread of its runtime.
async fn catalogue_of<S: Services>(cx: &LiveContext<S>) -> Result<Catalogue, Failure> {
    fn assert_send<F: Future + Send>(future: F) -> F {
        future
    }
    let (reply, events, _stop) = Reply::collecting();
    assert_send(library::load_catalogue(cx, REQUEST, &reply)).await;
    let sent: Vec<Event> = events.try_iter().collect();
    let [
        Event::Catalogue {
            request: REQUEST,
            result,
        },
    ] = sent.as_slice()
    else {
        panic!("expected one catalogue, for {REQUEST:?}: {sent:#?}");
    };
    result.clone()
}

/// The document that a chapter of three pages makes, as the catalogue shows it.
fn shown(summary: &IngestSummary, number: u32, name: &str, folder: &Path) -> Document {
    let counts = &summary.items_by_kind;
    Document {
        id: summary.doc_id.into(),
        title: summary.doc_title.clone(),
        chapter: Some(ChapterLabel {
            number,
            name: name.to_owned(),
        }),
        author: None,
        tags: Vec::new(),
        pages: Some(3),
        items: ItemCounts {
            chunks: counts.chunks as u64,
            formulas: counts.formulas as u64,
            figures: counts.figures as u64,
            tables: counts.tables as u64,
        },
        ingested_items: Some(counts.total() as u64),
        folder: Some(folder.to_path_buf()),
    }
}

/// A committed chapter, as a path that does not depend on where the test runs from.
fn sample_chapter(chapter: &str) -> PathBuf {
    let folder = gui::testkit::samples_folder().join(chapter);
    std::fs::canonicalize(folder).expect("a committed chapter should exist")
}

fn copy_folder(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("a folder should be made");
    for entry in std::fs::read_dir(from).expect("the folder should be listed") {
        let entry = entry.expect("an entry should be read");
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_folder(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("a file should be copied");
        }
    }
}
