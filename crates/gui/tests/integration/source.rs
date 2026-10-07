//! Checks that the live backend reads a page of a saved chapter and the concepts of that page.
// SMELL: this file is close to the limit of 500 lines. The test of the concepts on the real
// stores, with what it writes to them, is the part to move out before another test is added.

use std::path::{Path, PathBuf};

use graph::{ConceptNode, DocumentNode, GraphStore, ItemNode, Mention};
use gui::backend::fake::Fake;
use gui::backend::live::source::{self, PageTarget};
use gui::backend::live::{LiveContext, RealServices, Services};
use gui::backend::{Handler, Reply};
use gui::contract::{
    ChapterLabel, Command, DocId, Event, Failure, FailureKind, ImageRef, PageBox, PageConcept,
    PagePiece, PageView, PieceKind, RequestId,
};
use ocr::content::{FORMAT_VERSION, PageIndex, PieceEntry, page_folder_name};
use ocr::{ChapterIndex, FigureImage, ImageShows, PieceDetail};
use rag_core::{ConceptId, Config, DocumentLabels, ItemId, ItemKind};
use uuid::Uuid;

use crate::support;

const REQUEST: RequestId = RequestId(7);
const VOLATILITY: &str = "option-volatility-and-pricing/chapter-1";
const NOTES: &str = "quanty-sample-notes/chapter-1";
const NO_CONTENT_FOLDER: &str = "/no/such/content/folder";

#[tokio::test(flavor = "multi_thread")]
async fn a_converted_page_loads_with_its_picture_its_figure_rectangle_and_its_neighbours() {
    let chapter = sample_chapter(VOLATILITY);
    // The folder is spelled in a way that the pictures must not repeat.
    let spelled = chapter.join("..").join("chapter-1");

    let page = shown(&closed(Path::new(NO_CONTENT_FOLDER)), &spelled, 5).await;

    let picture = |page: u32| {
        let path = chapter.join(page_folder_name(page)).join("page.png");
        Some(ImageRef { path })
    };
    let text = |number, file: &str| PagePiece {
        number,
        kind: PieceKind::Text,
        label: None,
        name: None,
        caption: None,
        text: text_of(&chapter.join(page_folder_name(5)).join(file)),
        image: None,
        cut: None,
    };
    let figure = PagePiece {
        kind: PieceKind::Figure,
        label: Some("Figure 13-4".to_owned()),
        image: Some(ImageRef {
            path: chapter.join(page_folder_name(5)).join("01-figure.png"),
        }),
        cut: Some(PageBox {
            left: 15,
            top: 23,
            right: 905,
            bottom: 485,
        }),
        ..text(1, "01-figure.md")
    };
    let expected = PageView {
        doc: DocId::default(),
        page: 5,
        book: Some("Option Volatility and Pricing".to_owned()),
        chapter: Some(ChapterLabel {
            number: 1,
            name: "Sample Pages".to_owned(),
        }),
        page_count: 7,
        printed_page: Some("233".to_owned()),
        image: picture(5),
        previous_image: picture(4),
        next_image: picture(6),
        pieces: vec![
            figure,
            text(2, "02-text.md"),
            text(3, "03-text.md"),
            text(4, "04-text.md"),
        ],
    };
    assert_eq!(page, expected);
    // The piece file ends with a newline that the page does not carry.
    assert!(page.pieces[1].text.starts_with("Which spread is best?"));
    assert!(page.pieces[3].text.ends_with("increase volatility."));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_hand_written_page_loads_with_no_page_picture() {
    use PieceKind::Heading;
    let cx = closed(Path::new(NO_CONTENT_FOLDER));

    let page = shown(&cx, &sample_chapter("quanty-sample-notes/chapter-2"), 3).await;

    assert_eq!(page.book.as_deref(), Some("Quanty Sample Notes"));
    let chapter = ChapterLabel {
        number: 2,
        name: "Black Scholes In Depth".to_owned(),
    };
    assert_eq!(page.chapter, Some(chapter));
    assert_eq!(page.page_count, 3);
    assert_eq!(page.printed_page.as_deref(), Some("7"));
    assert_eq!(page.image, None);
    assert_eq!(page.previous_image, None);
    assert_eq!(page.next_image, None);
    assert!(page.pieces.iter().all(|piece| piece.image.is_none()));
    let numbers: Vec<u32> = page.pieces.iter().map(|piece| piece.number).collect();
    assert_eq!(numbers, (1..=10).collect::<Vec<_>>());
    assert_eq!(page.pieces[2].kind, Heading { rank: 2 });
    assert_eq!(page.pieces[2].text, "The Pricing Formulas");
    let formula = &page.pieces[4];
    assert_eq!(formula.kind, PieceKind::Formula);
    assert_eq!(formula.label.as_deref(), Some("(2.4)"));
    assert_eq!(formula.name.as_deref(), Some("Black–Scholes call price"));
    assert_eq!(formula.text, r"C = S\,N(d_1) - K e^{-rT} N(d_2)");

    // The label of a heading is its printed number, that of a table is its own, and that of a
    // footnote is its marker.
    let notes = sample_chapter(NOTES);
    let first_page = shown(&cx, &notes, 1).await;
    let title = &first_page.pieces[0];
    assert_eq!(title.kind, Heading { rank: 1 });
    assert_eq!(title.label.as_deref(), Some("1"));
    let table_page = shown(&cx, &notes, 2).await;
    let (table, footnote) = (&table_page.pieces[2], &table_page.pieces[5]);
    assert_eq!(table.kind, PieceKind::Table);
    assert_eq!(table.label.as_deref(), Some("Table 1-1"));
    let caption = "How the price of a call and of a put respond when one input rises.";
    assert_eq!(table.caption.as_deref(), Some(caption));
    assert_eq!(footnote.kind, PieceKind::Footnote);
    assert_eq!(footnote.label.as_deref(), Some("1"));
}

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
async fn a_document_with_no_folder_is_found_in_the_content_folder() {
    let samples = gui::testkit::samples_folder();
    // The content folder is spelled in a way that the pictures must not repeat.
    let cx = closed(&samples.join("..").join("content"));
    let chapter = sample_chapter(VOLATILITY);
    let index = ChapterIndex::read(&chapter).expect("the chapter index should be read");
    let doc = rag_core::DocId::from_source_sha256(&index.source_sha256).into();
    // A folder that was moved since the document was stored is searched for in the same way.
    let moved = samples.join("no-such-book/chapter-1");

    let given = loaded(&cx, &page_target(doc, 5, Some(&chapter))).await.0;
    let searched = loaded(&cx, &page_target(doc, 5, None)).await.0;
    let found_again = loaded(&cx, &page_target(doc, 5, Some(&moved))).await.0;

    let given = given.expect("the page should load from the folder it was given");
    assert_eq!(given.pieces.len(), 4);
    assert_eq!(searched.expect("the page should be found by its id"), given);
    assert_eq!(found_again.expect("the page should be found again"), given);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_page_that_cannot_be_shown_says_what_to_do() {
    // A document that no folder is known for.
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

    // A page that the chapter does not have, on either side.
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

/// A chapter that was never finished is reported as missing, with the folder that was read.
async fn an_unfinished_chapter_says_where_it_is() {
    let saved = tempfile::tempdir().expect("a temporary folder should be made");
    saved_chapter(saved.path(), false, ImageShows::Figure, None);
    let cx = closed(Path::new(NO_CONTENT_FOLDER));

    let failure = page_failure(&cx, &page_target(DocId::default(), 1, Some(saved.path()))).await;

    assert_eq!(failure.kind, FailureKind::SourceMissing);
    let folder = std::fs::canonicalize(saved.path()).expect("the folder should be there");
    let said = format!("{} {}", failure.hint, failure.detail);
    assert!(said.contains(&folder.display().to_string()), "{failure:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_figure_saved_as_the_whole_page_has_no_rectangle() {
    let saved = tempfile::tempdir().expect("a temporary folder should be made");
    let cx = closed(Path::new(NO_CONTENT_FOLDER));
    let rectangle = |right| ocr::PageBox {
        left: 15,
        top: 23,
        right,
        bottom: 485,
    };
    // A whole-page picture can be saved with the rectangle set. A rectangle past the edge of
    // the page cannot be drawn.
    for (shows, cut) in [
        (ImageShows::WholePage, rectangle(905)),
        (ImageShows::Figure, rectangle(1200)),
    ] {
        saved_chapter(saved.path(), true, shows, Some(cut));

        let page = shown(&cx, saved.path(), 1).await;

        assert_eq!(page.pieces[0].cut, None, "{shows:?} with {cut:?}");
        assert!(page.pieces[0].image.is_some(), "the picture is still sent");
    }

    // A picture whose file is gone is not sent.
    let gone = saved.path().join(page_folder_name(1)).join("01-figure.png");
    std::fs::remove_file(gone).expect("the picture should be removed");
    assert_eq!(shown(&cx, saved.path(), 1).await.pieces[0].image, None);
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
    for document in catalogue.documents() {
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

    // No chapter is on disk for this document, and the concepts come all the same.
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

/// A context whose stores are down and whose content folder is `content_folder`.
fn closed(content_folder: &Path) -> LiveContext<RealServices> {
    let config = Config {
        content_folder: content_folder.to_path_buf(),
        ..support::closed_ports_config()
    };
    LiveContext::new(config, RealServices)
}

/// A committed chapter, as the path that the page carries: ingestion keeps the canonical path of
/// a chapter folder, and the path of a test does not depend on where it runs from.
fn sample_chapter(chapter: &str) -> PathBuf {
    let folder = gui::testkit::samples_folder().join(chapter);
    std::fs::canonicalize(folder).expect("a committed chapter should exist")
}

/// The text of a piece file, without its one closing newline.
fn text_of(file: &Path) -> String {
    let text = std::fs::read_to_string(file).expect("a piece file should be read");
    text.strip_suffix('\n').unwrap_or(&text).to_owned()
}

fn page_target(doc: DocId, page: u32, folder: Option<&Path>) -> PageTarget {
    PageTarget {
        doc,
        page,
        folder: folder.map(Path::to_path_buf),
    }
}

/// Saves a chapter of one page in `folder`. The page holds one figure, whose picture shows
/// `shows` and was cut at `cut`, and the file of that picture is there.
fn saved_chapter(folder: &Path, finished: bool, shows: ImageShows, cut: Option<ocr::PageBox>) {
    let index = ChapterIndex {
        format_version: FORMAT_VERSION,
        book_title: "A Saved Book".to_owned(),
        chapter_number: 1,
        chapter_name: "One Figure".to_owned(),
        source_file: "chapter-1-one-figure.pdf".to_owned(),
        source_sha256: "a-made-up-hash".to_owned(),
        page_count: 1,
        finished,
    };
    index.write(folder).unwrap();
    let page_folder = folder.join(page_folder_name(1));
    std::fs::create_dir_all(&page_folder).unwrap();
    std::fs::write(page_folder.join("01-figure.png"), b"not a real picture").unwrap();
    let picture = FigureImage {
        file: "01-figure.png".to_owned(),
        shows,
        cut,
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

async fn shown(cx: &LiveContext<RealServices>, folder: &Path, page: u32) -> PageView {
    let (page, _concepts) = loaded(cx, &page_target(DocId::default(), page, Some(folder))).await;
    page.expect("the page should load")
}

async fn page_failure<S: Services>(cx: &LiveContext<S>, target: &PageTarget) -> Failure {
    let (page, _concepts) = loaded(cx, target).await;
    page.expect_err("the page should not load")
}

/// Loads the page and returns what the two events carry, after checking that exactly two were
/// sent, the page first and its concepts second, both with the id of the request. The `Send`
/// bound is a proof at compile time that the window can run the load on any thread.
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
