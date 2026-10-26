//! Checks that the live backend reads every item of a saved chapter from disk, with its saved
//! metadata when the metadata still matches the chapter.

use std::path::Path;

use backend::Handler;
use contract::{
    ChapterContents, Command, DocId, Event, Failure, FailureKind, PageConcept, PieceKind, RequestId,
};
use ocr::content::{FORMAT_VERSION, PageIndex, PieceEntry, page_folder_name};
use ocr::{ChapterIndex, PieceDetail};
use uuid::Uuid;

use crate::support::{IN_DEPTH, closed, copy_folder, sample_chapter, served};

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
    assert!(
        contents.items.iter().all(|item| item.metadata.is_none()),
        "source-stale metadata preserves original fallback content"
    );
    let source = ocr::read_chapter(saved.path()).unwrap().artifact_source();
    let texts = [
        source.manifest.first().unwrap(),
        source.manifest.last().unwrap(),
    ]
    .map(|target| ocr::content::ArtifactEntry {
        text: ocr::content::ArtifactText {
            id: target.id.clone(),
            title: "Prefetching with __builtin_prefetch".to_owned(),
            description: "Warms std::vector<InstrumentState> for the next event.".to_owned(),
        },
        model: "fixture".to_owned(),
        cache_identity: "fixture".to_owned(),
    });
    let mut metadata = ocr::content::ChapterArtifacts {
        format_version: 1,
        source,
        generation_signature: "old-policy".to_owned(),
        entries: texts.to_vec(),
    };
    metadata.write(saved.path()).unwrap();
    let (partial, _) = loaded(&cx, DocId::default(), Some(saved.path())).await;
    let partial = partial.unwrap();
    assert_eq!(
        partial
            .items
            .iter()
            .filter(|item| item.metadata.is_some())
            .count(),
        2
    );
    assert_eq!(
        partial
            .items
            .last()
            .unwrap()
            .metadata
            .as_ref()
            .unwrap()
            .title,
        "Prefetching with __builtin_prefetch",
        "multipart metadata joins by the first piece"
    );
    assert_eq!(partial.items.last().unwrap().piece, listing.piece);
    metadata.entries[0].text.title.clear();
    metadata.write(saved.path()).unwrap();
    let (invalid, _) = loaded(&cx, DocId::default(), Some(saved.path())).await;
    assert!(
        invalid
            .unwrap()
            .items
            .iter()
            .all(|item| item.metadata.is_none())
    );
    std::fs::write(saved.path().join("artifacts.json"), "broken json").unwrap();
    let (corrupt, _) = loaded(&cx, DocId::default(), Some(saved.path())).await;
    assert_eq!(corrupt.unwrap(), contents);

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
pub(super) async fn loaded(
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
