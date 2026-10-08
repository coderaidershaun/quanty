//! Checks that the live backend lists stored documents with their chapters on disk, keeps a media
//! saved before any document between starts, and writes new labels to both stores with no model
//! asked.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use graph::{GraphStore, MediaNode};
use gui::backend::live::{LiveContext, RealServices, Services};
use gui::backend::{Handler, Reply};
use gui::contract::{
    Catalogue, Category, ChapterLabel, Command, DocId, Document, DocumentTagsEdit, Event, Failure,
    FailureKind, ItemCounts, Media, MediaEdit, NewMedia, RequestId,
};
use rag_core::{DocumentLabels, ItemFilter, MediaLabels, Tag};
use rag_ingestion::testing::{StandInEmbedder, StandInLlm, ThrowawayStores, first_axis};
use rag_ingestion::{ChapterFolder, IngestSummary, ingest_chapter};
use uuid::Uuid;

use crate::support::{self, IN_DEPTH, INTUITION, SAMPLE_PAGES, copy_folder, sample_chapter};

const REQUEST: RequestId = RequestId(7);

/// The converted chapter in `folder`, whose media is made with no labels when it is new.
fn chapter_at(folder: &Path) -> ChapterFolder<'_> {
    static NO_LABELS: MediaLabels = MediaLabels {
        category: rag_core::Category::Book,
        authors: Vec::new(),
        tags: BTreeSet::new(),
    };
    ChapterFolder {
        folder,
        new_media: &NO_LABELS,
    }
}

fn owned(texts: &[&str]) -> Vec<String> {
    texts.iter().map(|text| (*text).to_owned()).collect()
}

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

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p gui --test integration -- --ignored library::"]
async fn the_labels_a_person_saves_are_in_the_next_catalogue_and_on_every_point_and_no_model_is_made()
 {
    let stores = ThrowawayStores::new("library-relabel");
    let connected = stores.connect().await;
    let models = stores.models(StandInLlm::finding_nothing());
    let intuition = sample_chapter(INTUITION);
    let stored = ingest_chapter(chapter_at(&intuition), &models, &connected).await;
    let stored = stored.expect("the chapter should be ingested");

    let answering = StandInLlm::finding_nothing();
    let embedders_made = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&embedders_made);
    let cx = support::context_with(stores, &answering, move || {
        counted.fetch_add(1, Ordering::SeqCst);
        StandInEmbedder::default()
    });

    let document = only_document_of(&cx).await;
    assert_eq!(
        (
            document.authors.len(),
            document.media_tags.len(),
            document.tags.len()
        ),
        (0, 0, 0)
    );

    let edit = MediaEdit {
        title: "quanty sample notes".to_owned(),
        category: Category::Paper,
        authors: owned(&["Sheldon Natenberg", "Euan Sinclair"]),
        tags: owned(&["Options"]),
    };
    assert_eq!(media_edited(&cx, &edit).await, Ok(()));
    let after_edit = only_document_of(&cx).await;
    assert_eq!(after_edit.authors, ["Sheldon Natenberg", "Euan Sinclair"]);
    assert_eq!(after_edit.media_tags, ["options"]);
    assert_eq!(after_edit.tags, Vec::<String>::new());
    let mut wanted = DocumentLabels {
        media: Some("Quanty Sample Notes".to_owned()),
        category: Some(rag_core::Category::Paper),
        authors: owned(&["Sheldon Natenberg", "Euan Sinclair"]),
        media_tags: tag_set(&["options"]),
        tags: BTreeSet::new(),
    };
    assert_every_point_carries(&cx, stored.doc_id, &wanted).await;

    let own = DocumentTagsEdit::toward(&after_edit, &owned(&["volatility", "Greeks", " "]));
    assert_eq!(document_tags_saved(&cx, &own).await, Ok(()));
    let after_tags = only_document_of(&cx).await;
    assert_eq!(after_tags.tags, ["greeks", "volatility"]);
    assert_eq!(
        (&after_tags.authors, &after_tags.media_tags),
        (&after_edit.authors, &after_edit.media_tags),
        "own tags leave the labels of the media alone"
    );
    wanted.tags = tag_set(&["greeks", "volatility"]);
    assert_every_point_carries(&cx, stored.doc_id, &wanted).await;

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

async fn one_answer<S: Services>(cx: &LiveContext<S>, command: Command) -> Event {
    let (reply, events, _stop) = Reply::collecting();
    cx.serve(command, reply).await;
    let mut sent: Vec<Event> = events.try_iter().collect();
    assert_eq!(sent.len(), 1, "expected one answer: {sent:#?}");
    sent.remove(0)
}

async fn catalogue_of<S: Services>(cx: &LiveContext<S>) -> Result<Catalogue, Failure> {
    let command = Command::LoadCatalogue { request: REQUEST };
    match one_answer(cx, command).await {
        Event::Catalogue {
            request: REQUEST,
            result,
        } => result,
        other => panic!("expected a catalogue, for {REQUEST:?}: {other:#?}"),
    }
}

async fn saved<S: Services>(cx: &LiveContext<S>, media: &NewMedia) -> Result<(), Failure> {
    let command = Command::SaveMedia {
        request: REQUEST,
        media: media.clone(),
    };
    match one_answer(cx, command).await {
        Event::MediaSaved {
            request: REQUEST,
            result,
        } => result,
        other => panic!("expected an answer to the save, for {REQUEST:?}: {other:#?}"),
    }
}

async fn media_edited<S: Services>(cx: &LiveContext<S>, edit: &MediaEdit) -> Result<(), Failure> {
    let command = Command::EditMedia {
        request: REQUEST,
        edit: edit.clone(),
    };
    match one_answer(cx, command).await {
        Event::MediaEdited {
            request: REQUEST,
            result,
        } => result,
        other => panic!("expected an answer to the edit, for {REQUEST:?}: {other:#?}"),
    }
}

async fn document_tags_saved<S: Services>(
    cx: &LiveContext<S>,
    edit: &DocumentTagsEdit,
) -> Result<(), Failure> {
    let command = Command::SetDocumentTags {
        request: REQUEST,
        edit: edit.clone(),
    };
    match one_answer(cx, command).await {
        Event::DocumentTagsSaved {
            request: REQUEST,
            doc,
            result,
        } => {
            assert_eq!(
                doc, edit.doc,
                "the answer names the document that was saved"
            );
            result
        }
        other => panic!("expected an answer to the save, for {REQUEST:?}: {other:#?}"),
    }
}

async fn only_document_of<S: Services>(cx: &LiveContext<S>) -> Document {
    let catalogue = catalogue_of(cx).await.expect("the catalogue is read");
    let mut documents = catalogue.documents().cloned();
    let document = documents.next().expect("the library holds a document");
    assert_eq!(documents.next(), None, "the library holds one document");
    document
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
