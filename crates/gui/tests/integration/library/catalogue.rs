//! Checks that the live backend lists stored documents with their chapters on disk, and keeps a
//! media saved before any document between starts.

use std::collections::BTreeSet;
use std::path::Path;

use graph::{GraphStore, MediaNode};
use gui::backend::live::{LiveContext, RealServices};
use gui::contract::{
    Catalogue, Category, ChapterLabel, Document, FailureKind, ItemCounts, Media, NewMedia,
};
use rag_core::MediaLabels;
use rag_ingestion::testing::chapter_at;
use rag_ingestion::{IngestSummary, ingest_chapter};

use super::{catalogue_of, owned, saved};
use crate::support::{self, IN_DEPTH, INTUITION, SAMPLE_PAGES, copy_folder, sample_chapter};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p gui --test integration -- --ignored library::"]
async fn the_catalogue_joins_stored_documents_with_their_chapter_folders() {
    let cx = support::context("library-join");
    let stores = cx.stores().await.expect("the stores should open");
    let models = cx.models().expect("the models should be made");
    let content = cx.config().content_folder.clone();
    // The first chapter is ingested where it is committed, and the graph keeps that folder.
    let intuition = sample_chapter(INTUITION);
    let first = ingest_chapter(chapter_at(&intuition), &models, &stores).await;
    let first = first.expect("the first chapter should be ingested");
    // The second chapter is ingested from a copy under the content folder, and the graph keeps a
    // folder that is not there, so the chapter must be found again by its id.
    let copy = content.join(IN_DEPTH);
    copy_folder(&sample_chapter(IN_DEPTH), &copy);
    let second = ingest_chapter(chapter_at(&copy), &models, &stores).await;
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
        media: vec![Media {
            title: Some("Quanty Sample Notes".to_owned()),
            category: Category::Book,
            authors: Vec::new(),
            tags: Vec::new(),
            documents: vec![
                shown(&first, 1, "Options Pricing Intuition", &intuition),
                shown(&second, 2, "Black Scholes In Depth", &copy),
            ],
        }],
    };
    assert_eq!(result, Ok(expected));
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p gui --test integration -- --ignored library::"]
async fn a_chapter_that_is_not_finished_is_not_taken_for_the_folder_of_its_document() {
    let cx = support::context("library-unfinished");
    let stores = cx.stores().await.expect("the stores should open");
    let models = cx.models().expect("the models should be made");
    let content = cx.config().content_folder.clone();
    let finished = content.join(IN_DEPTH);
    copy_folder(&sample_chapter(IN_DEPTH), &finished);
    let stored = ingest_chapter(chapter_at(&finished), &models, &stores).await;
    let stored = stored.expect("the chapter should be ingested");
    // A second conversion of the same PDF stopped half way, in a media folder that the scan of
    // the content folder reads first, and the graph keeps it as the document's folder.
    let unfinished = content.join("a-first-media").join("chapter-2");
    copy_folder(&sample_chapter(IN_DEPTH), &unfinished);
    let index = unfinished.join("chapter.json");
    let saved = std::fs::read_to_string(&index).expect("the copied index should be read");
    let stopped = saved.replace("\"finished\": true", "\"finished\": false");
    assert_ne!(
        stopped, saved,
        "the copied index should say that it is finished"
    );
    std::fs::write(&index, stopped).expect("the copied index should be written");
    stores
        .graph
        .set_chapter_folder(stored.doc_id, &unfinished)
        .await
        .expect("the folder should be stored");

    let catalogue = catalogue_of(&cx).await.expect("the catalogue is read");

    let document = catalogue
        .document(stored.doc_id.into())
        .expect("the chapter is listed");
    assert_eq!(document.folder.as_deref(), Some(finished.as_path()));
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p gui --test integration -- --ignored library::"]
async fn a_saved_book_is_listed_after_a_second_start_shows_its_chapter_and_is_not_saved_twice() {
    // Every save and every ingest goes to the first start, and every catalogue is read by the
    // second one, so a saved book that is only in the memory of the first is not found.
    let first = support::context("library-books");
    let stores = first.stores().await.expect("the stores should open");
    let models = first.models().expect("the models should be made");
    let sample_pages = sample_chapter(SAMPLE_PAGES);
    let stored = ingest_chapter(chapter_at(&sample_pages), &models, &stores).await;
    stored.expect("the first chapter should be ingested");

    let notes = NewMedia {
        title: "Quanty Sample Notes".to_owned(),
        category: Category::Paper,
        authors: owned(&[" Sheldon Natenberg ", "Euan Sinclair", " "]),
        tags: owned(&["Volatility", "options", " "]),
    };
    assert_eq!(saved(&first, &notes).await, Ok(()));
    // The commonest save: no authors and no tags are sent to the store as empty lists.
    let hedging = NewMedia {
        title: "Dynamic Hedging".to_owned(),
        ..NewMedia::default()
    };
    assert_eq!(saved(&first, &hedging).await, Ok(()));

    let second = support::started_again(&first);
    let before = catalogue_of(&second).await.expect("the catalogue is read");
    let authors = ["Sheldon Natenberg", "Euan Sinclair"];
    let hedging_media = ("Dynamic Hedging", Category::Book, &[][..], &[][..], 0);
    let pricing_media = (
        "Option Volatility and Pricing",
        Category::Book,
        &[][..],
        &[][..],
        1,
    );
    let notes_media = (
        "Quanty Sample Notes",
        Category::Paper,
        &authors[..],
        &["options", "volatility"][..],
        0,
    );
    assert_eq!(
        media_shown(&before),
        media_shown_as(&[hedging_media, pricing_media, notes_media])
    );

    let intuition = sample_chapter(INTUITION);
    let stored = ingest_chapter(chapter_at(&intuition), &models, &stores).await;
    let stored = stored.expect("the chapter of the saved media should be ingested");
    let notes_with_document = (
        notes_media.0,
        notes_media.1,
        notes_media.2,
        notes_media.3,
        1,
    );
    let after = catalogue_of(&second).await.expect("the catalogue is read");
    assert_eq!(
        media_shown(&after),
        media_shown_as(&[hedging_media, pricing_media, notes_with_document])
    );
    let document = after
        .document(stored.doc_id.into())
        .expect("the chapter is listed");
    assert_eq!(
        (&document.authors, &document.media_tags),
        (&owned(&authors), &owned(&["options", "volatility"])),
        "the chapter carries the labels of the saved media"
    );

    let shouted = NewMedia {
        title: " quanty SAMPLE notes ".to_owned(),
        ..NewMedia::default()
    };
    let other_capitals = NewMedia {
        title: "option volatility and pricing".to_owned(),
        ..NewMedia::default()
    };
    let no_folder_name = NewMedia {
        title: "!!!".to_owned(),
        ..NewMedia::default()
    };
    let refused = saved(&first, &shouted).await;
    let refused = refused.expect_err("a stored media is there already");
    assert_eq!(refused.kind, FailureKind::MediaExists);
    assert!(
        refused
            .hint
            .starts_with("Quanty Sample Notes is in the library"),
        "{refused:?}"
    );
    let refused = saved(&first, &other_capitals).await;
    let refused = refused.expect_err("the media of a stored chapter is there already");
    assert_eq!(refused.kind, FailureKind::MediaExists);
    let refused = saved(&first, &no_folder_name).await;
    let refused = refused.expect_err("no folder can be named after this title");
    assert_eq!(refused.kind, FailureKind::BadFile);

    let overwrite = MediaNode {
        title: "Quanty Sample Notes".to_owned(),
        labels: MediaLabels {
            category: rag_core::Category::Other,
            authors: vec!["Someone Else".to_owned()],
            tags: BTreeSet::new(),
        },
    };
    stores
        .graph
        .add_media(&overwrite)
        .await
        .expect("the graph accepts the same title again");
    let unchanged = catalogue_of(&second).await.expect("the catalogue is read");
    assert_eq!(unchanged, after, "a stored media is never written over");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_catalogue_answers_once_when_the_stores_are_down() {
    let cx = LiveContext::new(support::closed_ports_config(), RealServices);

    let result = catalogue_of(&cx).await;

    result.expect_err("no store answers, so the catalogue cannot be read");
}

type MediaShown = (Option<String>, Category, Vec<String>, Vec<String>, usize);

/// A media as a test writes it: title, category, authors, tags and number of documents.
type MediaWritten<'a> = (&'a str, Category, &'a [&'a str], &'a [&'a str], usize);

/// What a test compares of each media: title, category, authors, tags and number of documents.
fn media_shown(catalogue: &Catalogue) -> Vec<MediaShown> {
    catalogue
        .media
        .iter()
        .map(|media| {
            (
                media.title.clone(),
                media.category,
                media.authors.clone(),
                media.tags.clone(),
                media.documents.len(),
            )
        })
        .collect()
}

fn media_shown_as(media: &[MediaWritten<'_>]) -> Vec<MediaShown> {
    media
        .iter()
        .map(|(title, category, authors, media_tags, documents)| {
            (
                Some((*title).to_owned()),
                *category,
                owned(authors),
                owned(media_tags),
                *documents,
            )
        })
        .collect()
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
        authors: Vec::new(),
        media_tags: Vec::new(),
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
