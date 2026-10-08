//! Checks that the live backend lists the stored documents, each with the chapter that is on
//! disk for it, that a book saved before any chapter is kept between two starts of the app, and
//! that new labels of a document are written to both stores with no model asked.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use graph::{BookNode, GraphStore};
use gui::backend::live::{LiveContext, RealServices, Services, library};
use gui::backend::{Handler, Reply};
use gui::contract::{
    Book, Catalogue, ChapterLabel, Command, DocId, Document, Event, Failure, FailureKind,
    ItemCounts, LabelEdit, NewBook, RequestId,
};
use rag_core::{DocumentLabels, ItemFilter, Tag};
use rag_ingestion::testing::{StandInEmbedder, StandInLlm, ThrowawayStores, first_axis};
use rag_ingestion::{IngestSummary, ingest_chapter};
use uuid::Uuid;

use crate::support::{self, IN_DEPTH, INTUITION, SAMPLE_PAGES, copy_folder, sample_chapter};

const REQUEST: RequestId = RequestId(7);

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
            author: None,
            tags: Vec::new(),
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
async fn a_saved_book_is_listed_after_a_second_start_shows_its_chapter_and_is_not_saved_twice() {
    // Every save and every ingest goes to the first start, and every catalogue is read by the
    // second one, so a saved book that is only in the memory of the first is not found.
    let first = support::context("library-books");
    let stores = first.stores().await.expect("the stores should open");
    let models = first.models().expect("the models should be made");
    let sample_pages = sample_chapter(SAMPLE_PAGES);
    let stored = ingest_chapter(&sample_pages, &models, &stores).await;
    stored.expect("the first chapter should be ingested");

    let natenberg = NewBook {
        title: "Quanty Sample Notes".to_owned(),
        author: Some("Sheldon Natenberg".to_owned()),
        tags: vec![
            "Volatility".to_owned(),
            "options".to_owned(),
            " ".to_owned(),
        ],
    };
    assert_eq!(saved(&first, &natenberg).await, Ok(()));
    // The commonest save: no author and no tags are sent to the store as a null and an empty list.
    let hedging = NewBook {
        title: "Dynamic Hedging".to_owned(),
        ..NewBook::default()
    };
    assert_eq!(saved(&first, &hedging).await, Ok(()));

    let second = support::started_again(&first);
    let before = catalogue_of(&second).await.expect("the catalogue is read");
    let hedging_book = ("Dynamic Hedging", None, &[][..], 0);
    let pricing_book = ("Option Volatility and Pricing", None, &[][..], 1);
    let notes_book = (
        "Quanty Sample Notes",
        Some("Sheldon Natenberg"),
        &["options", "volatility"][..],
        0,
    );
    assert_eq!(
        books_shown(&before),
        books_shown_as(&[hedging_book, pricing_book, notes_book])
    );

    let intuition = sample_chapter(INTUITION);
    let stored = ingest_chapter(&intuition, &models, &stores).await;
    stored.expect("the chapter of the saved book should be ingested");
    let notes_with_chapter = ("Quanty Sample Notes", notes_book.1, notes_book.2, 1);
    let after = catalogue_of(&second).await.expect("the catalogue is read");
    assert_eq!(
        books_shown(&after),
        books_shown_as(&[hedging_book, pricing_book, notes_with_chapter])
    );

    let shouted = NewBook {
        title: " quanty SAMPLE notes ".to_owned(),
        ..NewBook::default()
    };
    let label_only = NewBook {
        title: "option volatility and pricing".to_owned(),
        ..NewBook::default()
    };
    let no_folder_name = NewBook {
        title: "!!!".to_owned(),
        ..NewBook::default()
    };
    let refused = saved(&first, &shouted).await;
    let refused = refused.expect_err("a stored book is there already");
    assert_eq!(refused.kind, FailureKind::BookExists);
    assert!(
        refused
            .hint
            .starts_with("Quanty Sample Notes is in the library"),
        "{refused:?}"
    );
    let refused = saved(&first, &label_only).await;
    let refused = refused.expect_err("a label on stored documents is there already");
    assert_eq!(refused.kind, FailureKind::BookExists);
    let refused = saved(&first, &no_folder_name).await;
    let refused = refused.expect_err("no chapter folder can be named after this title");
    assert_eq!(refused.kind, FailureKind::BadFile);

    let overwrite = BookNode {
        title: "Quanty Sample Notes".to_owned(),
        author: Some("Someone Else".to_owned()),
        tags: BTreeSet::new(),
    };
    stores
        .graph
        .add_book(&overwrite)
        .await
        .expect("the graph accepts the same title again");
    let unchanged = catalogue_of(&second).await.expect("the catalogue is read");
    assert_eq!(unchanged, after, "a stored book is never written over");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_catalogue_answers_once_when_the_stores_are_down() {
    let cx = LiveContext::new(support::closed_ports_config(), RealServices);

    let result = catalogue_of(&cx).await;

    result.expect_err("no store answers, so the catalogue cannot be read");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p gui --test integration -- --ignored library::"]
async fn the_labels_a_person_saves_are_in_the_next_catalogue_and_on_every_point_and_no_model_is_made()
 {
    let stores = ThrowawayStores::new("library-relabel");
    let connected = stores.connect().await;
    let models = stores.models(StandInLlm::finding_nothing());
    let intuition = sample_chapter(INTUITION);
    let stored = ingest_chapter(&intuition, &models, &connected).await;
    let stored = stored.expect("the chapter should be ingested");

    let answering = StandInLlm::finding_nothing();
    let embedders_made = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&embedders_made);
    let cx = support::context_with(stores, &answering, move || {
        counted.fetch_add(1, Ordering::SeqCst);
        StandInEmbedder::default()
    });

    let document = only_document_of(&cx).await;
    assert_eq!((document.author.as_deref(), document.tags.len()), (None, 0));

    let first = LabelEdit::toward(
        &document,
        Some("Sheldon Natenberg"),
        &["Options".to_owned(), "volatility".to_owned()],
    );
    assert_eq!(relabelled(&cx, &first).await, Ok(()));
    let after_first = only_document_of(&cx).await;
    assert_eq!(after_first.author.as_deref(), Some("Sheldon Natenberg"));
    assert_eq!(after_first.tags, ["options", "volatility"]);
    assert_every_point_carries(
        &cx,
        stored.doc_id,
        "Sheldon Natenberg",
        &["options", "volatility"],
    )
    .await;

    // A change of the author, a tag added and a tag taken away, in one save.
    let second = LabelEdit::toward(
        &after_first,
        Some("S. Natenberg"),
        &["volatility".to_owned(), "greeks".to_owned()],
    );
    assert_eq!(relabelled(&cx, &second).await, Ok(()));
    let after_second = only_document_of(&cx).await;
    assert_eq!(after_second.author.as_deref(), Some("S. Natenberg"));
    assert_eq!(after_second.tags, ["greeks", "volatility"]);
    assert_every_point_carries(
        &cx,
        stored.doc_id,
        "S. Natenberg",
        &["greeks", "volatility"],
    )
    .await;

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
    let edit = LabelEdit {
        doc: DocId(Uuid::from_u128(1)),
        author: Some("Sheldon Natenberg".to_owned()),
        ..LabelEdit::default()
    };

    let result = relabelled(&cx, &edit).await;

    let failure = result.expect_err("no store answers, so no label can be written");
    assert!(
        matches!(
            failure.kind,
            FailureKind::FalkorDbDown | FailureKind::QdrantDown
        ),
        "{failure:?}"
    );
}

/// The `Send` bound is a proof at compile time that the window can run the load on any thread of
/// its runtime.
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

/// Sends the save of one book and returns its one answer.
async fn saved<S: Services>(cx: &LiveContext<S>, book: &NewBook) -> Result<(), Failure> {
    let (reply, events, _stop) = Reply::collecting();
    let command = Command::SaveBook {
        request: REQUEST,
        book: book.clone(),
    };
    cx.serve(command, reply).await;
    let sent: Vec<Event> = events.try_iter().collect();
    let [
        Event::BookSaved {
            request: REQUEST,
            result,
        },
    ] = sent.as_slice()
    else {
        panic!("expected one answer to the save, for {REQUEST:?}: {sent:#?}");
    };
    result.clone()
}

/// Sends the save of one change of labels and returns its one answer.
async fn relabelled<S: Services>(cx: &LiveContext<S>, edit: &LabelEdit) -> Result<(), Failure> {
    let (reply, events, _stop) = Reply::collecting();
    let command = Command::SetLabels {
        request: REQUEST,
        edit: edit.clone(),
    };
    cx.serve(command, reply).await;
    let sent: Vec<Event> = events.try_iter().collect();
    let [
        Event::LabelsSaved {
            request: REQUEST,
            doc,
            result,
        },
    ] = sent.as_slice()
    else {
        panic!("expected one answer to the save, for {REQUEST:?}: {sent:#?}");
    };
    assert_eq!(
        *doc, edit.doc,
        "the answer names the document that was saved"
    );
    result.clone()
}

async fn only_document_of<S: Services>(cx: &LiveContext<S>) -> Document {
    let catalogue = catalogue_of(cx).await.expect("the catalogue is read");
    let mut documents = catalogue.documents().cloned();
    let document = documents.next().expect("the library holds a document");
    assert_eq!(documents.next(), None, "the library holds one document");
    document
}

/// Every point of the document, not only the nearest ones, has these labels and its book.
async fn assert_every_point_carries<S: Services>(
    cx: &LiveContext<S>,
    document: rag_core::DocId,
    author: &str,
    tags: &[&str],
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
    let wanted = DocumentLabels {
        book: Some("Quanty Sample Notes".to_owned()),
        author: Some(author.to_owned()),
        tags: tags
            .iter()
            .map(|tag| tag.parse::<Tag>().expect("a tag is not blank"))
            .collect(),
    };
    for hit in hits {
        assert_eq!(hit.payload.document_labels, wanted, "point {}", hit.id);
    }
}

type BookShown = (Option<String>, Option<String>, Vec<String>, usize);

/// What a test compares of each book: title, saved author, saved tags and number of chapters.
fn books_shown(catalogue: &Catalogue) -> Vec<BookShown> {
    catalogue
        .books
        .iter()
        .map(|book| {
            (
                book.title.clone(),
                book.author.clone(),
                book.tags.clone(),
                book.chapters.len(),
            )
        })
        .collect()
}

fn books_shown_as(books: &[(&str, Option<&str>, &[&str], usize)]) -> Vec<BookShown> {
    books
        .iter()
        .map(|(title, author, tags, chapters)| {
            (
                Some((*title).to_owned()),
                author.map(str::to_owned),
                tags.iter().map(|tag| (*tag).to_owned()).collect(),
                *chapters,
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
