//! Checks that the live backend reads every item of a saved chapter from disk and the concepts of
//! the document from the graph, and that the fake backend lists what the live reader lists.

use std::path::Path;

use graph::{ConceptNode, DocumentNode, ItemNode, Mention};
use gui::backend::Handler;
use gui::backend::fake::Fake;
use gui::contract::{
    ChapterContents, Command, DocId, Event, Failure, FailureKind, PageConcept, PieceKind, RequestId,
};
use ocr::content::{FORMAT_VERSION, PageIndex, PieceEntry, page_folder_name};
use ocr::{ChapterIndex, PieceDetail};
use rag_core::{ConceptId, DocumentLabels, ItemId, ItemKind};
use uuid::Uuid;

use crate::support::{self, IN_DEPTH, closed, copy_folder, sample_chapter, served};

const REQUEST: RequestId = RequestId(7);
const NO_CONTENT_FOLDER: &str = "/no/such/content/folder";

#[tokio::test(flavor = "multi_thread")]
async fn the_items_of_a_chapter_load_from_its_folder_while_the_stores_are_down() {
    let sample = sample_chapter(IN_DEPTH);
    let saved = tempfile::tempdir().expect("a temporary folder should be made");
    copy_folder(&sample, saved.path());
    let continued = "second = 2\nthird = 3";
    add_page_with_code(saved.path(), continued);
    let cx = closed(Path::new(NO_CONTENT_FOLDER));

    let (items, concepts) = loaded(&cx, DocId::default(), Some(saved.path())).await;

    let contents = items.expect("the items need no store");
    assert_eq!(contents.page_count, 4);
    let labels: Vec<&str> = contents
        .items
        .iter()
        .map(|item| item.piece.label.as_deref().unwrap_or("none"))
        .collect();
    assert_eq!(
        labels,
        [
            "(2.1)",
            "(2.2)",
            "(2.3)",
            "(2.4)",
            "(2.5)",
            "(2.6)",
            "Listing 2.1"
        ],
        "formulas, then the one listing, in reading order"
    );
    let listing = contents
        .items
        .last()
        .expect("the chapter should have a listing");
    assert_eq!(listing.piece.kind, PieceKind::Code);
    assert_eq!((listing.page, listing.last_page), (3, Some(4)));
    assert_eq!(
        (
            listing.printed_page.as_deref(),
            listing.last_printed_page.as_deref()
        ),
        (Some("7"), Some("8")),
        "a listing that a page break cut is one item from its first page to its last"
    );
    let first_part = sample.join(page_folder_name(3)).join("11-code.txt");
    let first_part = std::fs::read_to_string(first_part).expect("the sample listing should exist");
    let first_part = first_part.strip_suffix('\n').unwrap_or(&first_part);
    assert_eq!(listing.piece.text, format!("{first_part}\n{continued}"));
    assert_eq!(listing.piece.language.as_deref(), Some("python"));

    let failure = concepts.expect_err("the graph is down, so there are no concepts");
    assert_eq!(failure.kind, FailureKind::FalkorDbDown);

    let content = tempfile::tempdir().expect("a temporary folder should be made");
    let unknown = DocId(Uuid::from_u128(9));
    let (items, _concepts) = loaded(&closed(content.path()), unknown, None).await;
    let failure = items.expect_err("no folder holds this document");
    assert_eq!(failure.kind, FailureKind::SourceMissing);
    let searched = content.path().display().to_string();
    assert_eq!(
        failure.hint,
        format!(
            "quanty does not know where this document's pages are. Put its folder inside its media's folder under {searched}, or ingest its PDF again with rag-ingest pdf."
        )
    );
    assert!(failure.detail.contains(&unknown.0.to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_fake_backend_lists_the_contents_of_every_sample_as_the_live_reader_does() {
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
    // The paper of the samples has no folder, so neither reader has items of it.
    let with_folders = catalogue
        .documents()
        .filter(|document| document.folder.is_some());
    for document in with_folders {
        let folder = document.folder.as_deref();
        let (faked, _concepts) = loaded(&fake, document.id, folder).await;
        let (live, _concepts) = loaded(&cx, document.id, folder).await;
        assert_eq!(
            faked.expect("the fake should list the contents"),
            live.expect("the live reader should list the contents"),
            "{}",
            document.title
        );
        compared += 1;
    }
    assert_eq!(compared, 3, "three chapters");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p gui --test integration -- --ignored contents::"]
async fn the_concepts_of_a_document_come_from_the_graph_most_mentioned_first() {
    let cx = support::context("contents-concepts");
    let stores = cx.stores().await.expect("the stores should open");
    let graph = &stores.graph;
    let document = rag_core::DocId::from_source_sha256("contents-concepts-document");
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
    // Volatility comes last by name, so a list in the order of the names would put it last.
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
        (second, &arbitrage),
        (third, &volatility),
        (third, &straddle),
    ]
    .map(|(item, concept)| Mention {
        item: item.id,
        concept: concept.id,
        wording: concept.name.clone(),
    });
    graph.add_mentions(&mentions).await.unwrap();

    let (items, concepts) = loaded(&cx, document.into(), None).await;

    let failure = items.expect_err("no chapter is saved for this document");
    assert_eq!(failure.kind, FailureKind::SourceMissing);
    let listed = |concept: &ConceptNode| PageConcept {
        id: concept.id.into(),
        name: concept.name.clone(),
        definition: concept.definition.clone(),
    };
    let expected = [listed(&volatility), listed(&arbitrage), listed(&straddle)];
    assert_eq!(concepts.expect("the concepts should be read"), expected);
}

/// Adds a fourth page with one code piece to a copy of a three page chapter, so the listing at the
/// end of page 3 goes on over the page break.
fn add_page_with_code(chapter: &Path, code: &str) {
    let page_folder = chapter.join(page_folder_name(4));
    std::fs::create_dir_all(&page_folder).expect("a page folder should be made");
    let detail = PieceDetail::Code {
        label: None,
        caption: None,
        language: Some("python".to_owned()),
        summary: "The rest of the listing.".to_owned(),
    };
    let entry = PieceEntry::new(1, detail);
    std::fs::write(page_folder.join(&entry.file), format!("{code}\n"))
        .expect("the code should be written");
    let page = PageIndex {
        format_version: FORMAT_VERSION,
        page_position: 4,
        printed_page_number: Some("8".to_owned()),
        running_header: None,
        starts_mid_sentence: false,
        ends_mid_sentence: false,
        pieces: vec![entry],
        relationships: Vec::new(),
        conversion: None,
    };
    page.write(&page_folder).expect("the page should be saved");
    let mut index = ChapterIndex::read(chapter).expect("the chapter index should be read");
    index.page_count = 4;
    index.write(chapter).expect("the chapter should be saved");
}

/// The items, then the concepts, each for `REQUEST`, as the window takes them.
async fn loaded(
    backend: &impl Handler,
    doc: DocId,
    folder: Option<&Path>,
) -> (
    Result<ChapterContents, Failure>,
    Result<Vec<PageConcept>, Failure>,
) {
    let command = Command::LoadChapterContents {
        request: REQUEST,
        doc,
        folder: folder.map(Path::to_path_buf),
    };
    let sent = served(backend, command).await;
    let [
        Event::ChapterContents {
            request: REQUEST,
            result: items,
        },
        Event::ChapterConcepts {
            request: REQUEST,
            result: concepts,
        },
    ] = sent.as_slice()
    else {
        panic!("expected the items and then the concepts, each for {REQUEST:?}: {sent:#?}");
    };
    (items.clone(), concepts.clone())
}
