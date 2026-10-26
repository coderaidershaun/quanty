//! Checks that a page of a saved chapter that cannot be shown says what to do.

use std::path::Path;

use backend::Reply;
use backend::live::source::{self, PageTarget};
use backend::live::{LiveContext, Services};
use contract::{DocId, Event, Failure, FailureKind, PageConcept, PageView, RequestId};
use ocr::content::{FORMAT_VERSION, PageIndex, PieceEntry, page_folder_name};
use ocr::{ChapterIndex, DocumentName, FigureImage, ImageShows, PieceDetail};
use uuid::Uuid;

use crate::support::{closed, sample_chapter};

const REQUEST: RequestId = RequestId(7);
const VOLATILITY: &str = "option-volatility-and-pricing/chapter-1";
const NO_CONTENT_FOLDER: &str = "/no/such/content/folder";

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
            "quanty does not know where this document's pages are. Put its folder inside its media's folder under {searched}, or ingest its PDF again with rag-ingest pdf."
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
                "This document has 7 pages, so page {page} is not in it. Ingest its PDF again with rag-ingest pdf."
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
        shows: ImageShows::Figure {
            cut: None,
            holds_body_text: false,
            unchecked: false,
        },
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
