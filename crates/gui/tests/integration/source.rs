//! Checks that the live backend reads a page of a saved chapter and the concepts of that page.

use std::path::Path;

use graph::{ConceptNode, DocumentNode, GraphStore, ItemNode, Mention};
use gui::backend::fake::Fake;
use gui::backend::live::source::{self, PageTarget};
use gui::backend::live::{LiveContext, Services};
use gui::backend::{Handler, Reply};
use gui::contract::{
    Command, DocId, Event, Failure, FailureKind, PageConcept, PageView, RequestId,
};
use ocr::content::{FORMAT_VERSION, PageIndex, PieceEntry, page_folder_name};
use ocr::{ChapterIndex, DocumentName, FigureImage, ImageShows, PieceDetail};
use rag_core::{ConceptId, DocumentLabels, ItemId, ItemKind};
use uuid::Uuid;

use crate::support::{self, closed, sample_chapter};

const REQUEST: RequestId = RequestId(7);
const VOLATILITY: &str = "option-volatility-and-pricing/chapter-1";
const NO_CONTENT_FOLDER: &str = "/no/such/content/folder";

#[tokio::test(flavor = "multi_thread")]
async fn the_page_loads_with_the_stores_down_and_the_concepts_say_why() {
    let cx = closed(Path::new(NO_CONTENT_FOLDER));
    let target = page_target(DocId::default(), 5, Some(&sample_chapter(VOLATILITY)));

    let (page, concepts) = loaded(&cx, &target).await;

    assert_eq!(page.expect("the page needs no store").page, 5);
    let failure = concepts.expect_err("the graph is down, so there are no concepts");
    assert_eq!(failure.kind, FailureKind::FalkorDbDown);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_page_that_cannot_be_shown_says_what_to_do() {
    let content = tempfile::tempdir().expect("a temporary folder should be made");
    let unknown = page_target(DocId(Uuid::from_u128(9)), 1, None);
    let failure = page_failure(&closed(content.path()), &unknown).await;
    assert_eq!(failure.kind, FailureKind::SourceMissing);
    let searched = content.path().display().to_string();
    assert_eq!(
        failure.hint,
        format!(
            "quanty does not know where this chapter's pages are. Put its chapter folder inside a book folder under {searched}, or ingest its PDF again with rag-ingest pdf."
        )
    );
    assert!(failure.detail.contains(&unknown.doc.0.to_string()));
    assert!(failure.detail.contains(&searched));

    let cx = closed(Path::new(NO_CONTENT_FOLDER));
    for page in [0, 8] {
        let chapter = sample_chapter(VOLATILITY);
        let failure = page_failure(&cx, &page_target(DocId::default(), page, Some(&chapter))).await;
        assert_eq!(failure.kind, FailureKind::SourceMissing);
        assert_eq!(
            failure.hint,
            format!(
                "This chapter has 7 pages, so page {page} is not in it. Run rag-ingest on its chapter folder again."
            )
        );
    }

    an_unfinished_chapter_says_where_it_is().await;
}

async fn an_unfinished_chapter_says_where_it_is() {
    let saved = tempfile::tempdir().expect("a temporary folder should be made");
    saved_chapter(saved.path());
    let cx = closed(Path::new(NO_CONTENT_FOLDER));

    let failure = page_failure(&cx, &page_target(DocId::default(), 1, Some(saved.path()))).await;

    assert_eq!(failure.kind, FailureKind::SourceMissing);
    let folder = std::fs::canonicalize(saved.path()).expect("the folder should be there");
    let said = format!("{} {}", failure.hint, failure.detail);
    assert!(said.contains(&folder.display().to_string()), "{failure:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_fake_backend_shows_every_sample_page_as_the_live_reader_does() {
    let home = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fake = Fake::scene("idle", home)
        .expect("the scene should start")
        .instant();
    let events = served(&fake, Command::LoadCatalogue { request: REQUEST }).await;
    let [
        Event::Catalogue {
            result: Ok(catalogue),
            ..
        },
        ..,
    ] = events.as_slice()
    else {
        panic!("expected the catalogue of the samples first: {events:#?}");
    };
    let cx = closed(Path::new(NO_CONTENT_FOLDER));

    let mut compared = 0;
    // The paper of the samples has no folder, so neither reader has a page of it.
    let with_folders = catalogue
        .documents()
        .filter(|document| document.folder.is_some());
    for document in with_folders {
        let pages = document.pages.expect("a sample has a page count");
        for page in 1..=pages {
            let (doc, folder) = (document.id, document.folder.clone());
            let command = Command::LoadPage {
                request: REQUEST,
                doc,
                page,
                folder: folder.clone(),
            };
            let events = served(&fake, command).await;
            let [Event::Page { result: faked, .. }, ..] = events.as_slice() else {
                panic!("expected the page first: {events:#?}");
            };
            let (live, _concepts) = loaded(&cx, &PageTarget { doc, page, folder }).await;
            assert_eq!(
                faked.as_ref().expect("the fake should show the page"),
                &live.expect("the live reader should show the page"),
                "{} page {page}",
                document.title
            );
            compared += 1;
        }
    }
    assert_eq!(compared, 13, "three chapters of 7, 3 and 3 pages");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p gui --test integration -- --ignored source::"]
async fn a_page_lists_the_concepts_its_items_mention() {
    let cx = support::context("source-concepts");
    let stores = cx.stores().await.expect("the stores should open");
    let graph = &stores.graph;
    let document = rag_core::DocId::from_source_sha256("source-concepts-document");
    let item = |kind, position, page| ItemNode {
        id: ItemId::new(document, kind, position),
        kind,
        page,
        printed_page: None,
    };
    let items = [
        item(ItemKind::Chunk, 0, 2),
        item(ItemKind::Formula, 1, 2),
        item(ItemKind::Chunk, 2, 3),
    ];
    let [first, second, third] = &items;
    let concept = |name: &str| ConceptNode {
        id: ConceptId::random(),
        name: name.to_owned(),
        normalised_name: name.to_lowercase(),
        definition: format!("{name} in one line"),
    };
    // "Arbitrage" comes before "Volatility" by name, so a list in the order of the names shows.
    let volatility = concept("Volatility");
    let arbitrage = concept("Arbitrage");
    let straddle = concept("Straddle");
    let node = DocumentNode {
        id: document,
        title: "A Document".to_owned(),
        labels: DocumentLabels::default(),
    };
    graph.upsert_document(&node).await.unwrap();
    graph.upsert_items(document, &items).await.unwrap();
    for stored in [&volatility, &arbitrage, &straddle] {
        graph.upsert_concept(stored).await.unwrap();
    }
    let mentions = [
        (first, &volatility),
        (second, &volatility),
        (second, &arbitrage),
        (third, &straddle),
    ]
    .map(|(item, concept)| Mention {
        item: item.id,
        concept: concept.id,
        wording: concept.name.clone(),
    });
    graph.add_mentions(&mentions).await.unwrap();
    let target = page_target(document.into(), 2, None);

    let (page, concepts) = loaded(&cx, &target).await;

    let failure = page.expect_err("no chapter is saved for this document");
    assert_eq!(failure.kind, FailureKind::SourceMissing);
    let listed = |concept: &ConceptNode| PageConcept {
        id: concept.id.into(),
        name: concept.name.clone(),
        definition: concept.definition.clone(),
    };
    let expected = [listed(&volatility), listed(&arbitrage)];
    assert_eq!(concepts.expect("the concepts should be read"), expected);
}

fn page_target(doc: DocId, page: u32, folder: Option<&Path>) -> PageTarget {
    PageTarget {
        doc,
        page,
        folder: folder.map(Path::to_path_buf),
    }
}

fn saved_chapter(folder: &Path) {
    let index = ChapterIndex {
        format_version: FORMAT_VERSION,
        media_title: "A Saved Book".to_owned(),
        name: DocumentName::Chapter {
            number: 1,
            name: "One Figure".to_owned(),
        },
        source_file: "chapter-1-one-figure.pdf".to_owned(),
        source_sha256: "a-made-up-hash".to_owned(),
        page_count: 1,
        finished: false,
    };
    index.write(folder).unwrap();
    let page_folder = folder.join(page_folder_name(1));
    std::fs::create_dir_all(&page_folder).unwrap();
    std::fs::write(page_folder.join("01-figure.png"), b"not a real picture").unwrap();
    let picture = FigureImage {
        file: "01-figure.png".to_owned(),
        shows: ImageShows::Figure,
        cut: None,
        holds_body_text: false,
        unchecked: false,
    };
    let figure = PieceDetail::Figure {
        label: Some("Figure 1-1".to_owned()),
        caption: None,
        printed_text: Vec::new(),
        bounds: None,
        image: Some(picture),
    };
    let entry = PieceEntry::new(1, figure);
    std::fs::write(page_folder.join(&entry.file), "A figure.\n").unwrap();
    let page = PageIndex {
        format_version: FORMAT_VERSION,
        page_position: 1,
        printed_page_number: None,
        running_header: None,
        starts_mid_sentence: false,
        ends_mid_sentence: false,
        pieces: vec![entry],
        relationships: Vec::new(),
        conversion: None,
    };
    page.write(&page_folder).unwrap();
}

async fn served(backend: &impl Handler, command: Command) -> Vec<Event> {
    let (reply, events, _stop) = Reply::collecting();
    backend.serve(command, reply).await;
    events.try_iter().collect()
}

async fn page_failure<S: Services>(cx: &LiveContext<S>, target: &PageTarget) -> Failure {
    let (page, _concepts) = loaded(cx, target).await;
    page.expect_err("the page should not load")
}

/// The `Send` bound is a proof at compile time that the window can run the load on any thread.
async fn loaded<S: Services>(
    cx: &LiveContext<S>,
    target: &PageTarget,
) -> (Result<PageView, Failure>, Result<Vec<PageConcept>, Failure>) {
    fn sendable<F: Future + Send>(future: F) -> F {
        future
    }
    let (reply, events, _stop) = Reply::collecting();
    sendable(source::load_page(cx, REQUEST, target, &reply)).await;
    let sent: Vec<Event> = events.try_iter().collect();
    let [
        Event::Page {
            request: REQUEST,
            result: page,
        },
        Event::PageConcepts {
            request: REQUEST,
            result: concepts,
        },
    ] = sent.as_slice()
    else {
        panic!("expected the page and then its concepts, each for {REQUEST:?}: {sent:#?}");
    };
    (page.clone(), concepts.clone())
}
