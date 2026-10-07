//! Checks that the live backend checks and runs an ingest of a chapter PDF: a new chapter is
//! converted with stand-in pages, stored and labelled, a file that cannot be ingested is refused
//! before any store is asked, and a page that fails or a store that is down ends the run as a
//! failure with nothing paid for.

use std::path::PathBuf;

use gui::backend::live::{LiveContext, Services};
use gui::backend::{Handler, Reply};
use gui::contract::{
    Catalogue, ChapterLabel, ChapterState, Command, Event, Failure, FailureKind, IngestOutcome,
    IngestProgress, IngestRequest, IngestStage, Preflight, RequestId,
};
use ocr::testing::{Scenario, sample_pdf};

use crate::support;

const REQUEST: RequestId = RequestId(7);

fn request(pdf: PathBuf, book: &str) -> IngestRequest {
    IngestRequest {
        pdf,
        book: book.to_owned(),
        author: None,
        tags: Vec::new(),
    }
}

async fn events_of<S: Services>(cx: &LiveContext<S>, command: Command) -> Vec<Event> {
    let (reply, events, _stop) = Reply::collecting();
    cx.serve(command, reply).await;
    events.try_iter().collect()
}

/// The result of a check, after checking that exactly one event was sent, for this request.
async fn checked<S: Services>(
    cx: &LiveContext<S>,
    ingest: &IngestRequest,
) -> Result<Preflight, Failure> {
    let command = Command::Preflight {
        request: REQUEST,
        ingest: ingest.clone(),
    };
    let events = events_of(cx, command).await;
    let [
        Event::Preflight {
            request: REQUEST,
            result,
        },
    ] = events.as_slice()
    else {
        panic!("expected one preflight for {REQUEST:?}: {events:#?}");
    };
    result.clone()
}

/// The progress of a start, in the order it was sent, and how the start ended. The end must be the
/// last event, and nothing but progress may come before it.
async fn started<S: Services>(
    cx: &LiveContext<S>,
    ingest: &IngestRequest,
) -> (Vec<IngestProgress>, Result<IngestOutcome, Failure>) {
    let command = Command::Ingest {
        request: REQUEST,
        ingest: ingest.clone(),
    };
    let events = events_of(cx, command).await;
    let Some((
        Event::IngestFinished {
            request: REQUEST,
            result,
        },
        before,
    )) = events.split_last()
    else {
        panic!("the last event must end the ingest of {REQUEST:?}: {events:#?}");
    };
    let progress = before
        .iter()
        .map(|event| match event {
            Event::IngestProgress {
                request: REQUEST,
                progress,
            } => *progress,
            other => panic!("only progress comes before the end: {other:#?}"),
        })
        .collect();
    (progress, result.clone())
}

async fn catalogue_of<S: Services>(cx: &LiveContext<S>) -> Catalogue {
    let command = Command::LoadCatalogue { request: REQUEST };
    let events = events_of(cx, command).await;
    let [
        Event::Catalogue {
            request: REQUEST,
            result,
        },
    ] = events.as_slice()
    else {
        panic!("expected one catalogue for {REQUEST:?}: {events:#?}");
    };
    result.clone().expect("the catalogue should be read")
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p gui --test integration -- --ignored ingest::"]
async fn a_checked_chapter_is_ingested_with_its_labels_and_a_second_start_finds_it_ingested() {
    let (cx, pages) = support::context_and_pages("ingest-run", Scenario::SampleChapter);
    let wanted = IngestRequest {
        author: Some(" Sheldon Natenberg ".to_owned()),
        tags: vec![
            "Options".to_owned(),
            "volatility".to_owned(),
            " ".to_owned(),
        ],
        ..request(sample_pdf(), "Option Volatility and Pricing")
    };

    let before = checked(&cx, &wanted).await;

    let new = Preflight {
        chapter: ChapterLabel {
            number: 1,
            name: "Sample Pages".to_owned(),
        },
        pages: None,
        state: Some(ChapterState::New),
        blockers: Vec::new(),
    };
    assert_eq!(before, Ok(new));

    let (progress, finished) = started(&cx, &wanted).await;

    let stages: Vec<IngestStage> = progress.iter().map(|progress| progress.stage).collect();
    assert_eq!(stages, [IngestStage::Converting, IngestStage::WritingGraph]);
    let Ok(IngestOutcome::Ingested(report)) = finished else {
        panic!("expected the chapter to be ingested: {finished:#?}");
    };
    assert_eq!(report.pages, 7);
    let counts = report.items;
    let items = counts.chunks + counts.formulas + counts.figures + counts.tables;
    assert!(items > 0, "{report:#?}");
    assert!(
        report.cost_usd.is_some(),
        "pages were converted in this run"
    );
    let catalogue = catalogue_of(&cx).await;
    let book = catalogue
        .book_of(report.doc)
        .expect("the document is in a book");
    assert_eq!(book.title.as_deref(), Some("Option Volatility and Pricing"));
    let stored = catalogue
        .document(report.doc)
        .expect("the document is stored");
    assert_eq!(stored.author.as_deref(), Some("Sheldon Natenberg"));
    assert_eq!(stored.tags, ["options", "volatility"]);

    let after = checked(&cx, &wanted).await;

    assert_eq!(
        after.map(|preflight| (preflight.state, preflight.blockers)),
        Ok((Some(ChapterState::Ingested { items }), Vec::new()))
    );
    let calls_so_far = pages.calls();
    assert!(!calls_so_far.is_empty(), "the first start converted pages");

    let (progress, again) = started(&cx, &wanted).await;

    assert!(progress.is_empty(), "nothing is converted: {progress:?}");
    assert_eq!(
        again,
        Ok(IngestOutcome::AlreadyIngested {
            doc: report.doc,
            items,
            pages_to_check: None,
        })
    );
    assert_eq!(pages.calls(), calls_so_far, "no page was converted again");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p gui --test integration -- --ignored ingest::"]
async fn a_page_that_fails_ends_the_run_as_failed_with_the_page_named_and_no_document_stored() {
    // Every reply for page 1 fails the check, so the conversion stops after its calls.
    let scenario = Scenario::AllTables {
        broken_page: Some(1),
    };
    let (cx, _pages) = support::context_and_pages("ingest-page-fails", scenario);
    let wanted = request(sample_pdf(), "Option Volatility and Pricing");

    let (progress, finished) = started(&cx, &wanted).await;

    let stages: Vec<IngestStage> = progress.iter().map(|progress| progress.stage).collect();
    assert_eq!(stages, [IngestStage::Converting]);
    let failure = finished.expect_err("a page that fails ends the run");
    assert_eq!(failure.kind, FailureKind::PageFailed, "{failure:?}");
    assert!(failure.hint.contains("Page 1"), "{failure:?}");
    let catalogue = catalogue_of(&cx).await;
    assert_eq!(catalogue.documents().count(), 0, "no document was stored");
}

#[tokio::test]
async fn a_file_the_check_must_refuse_is_refused_before_any_store_is_asked() {
    let folder = tempfile::tempdir().expect("a temporary folder should be made");
    let file = |name: &str, bytes: &[u8]| -> PathBuf {
        let path = folder.path().join(name);
        std::fs::write(&path, bytes).expect("a file should be written");
        path
    };
    let wrong_name = file("notes.pdf", b"%PDF-1.4 a PDF with the wrong name");
    let plain_text = file("chapter-1-text.pdf", b"This is plain text, not a PDF.");
    let right_file = file("chapter-2-fine.pdf", b"%PDF-1.4 a PDF with the right name");
    let cx = support::closed(&folder.path().join("content"));
    let refused = [
        (
            request(wrong_name, "A Test Book"),
            "chapter-<number>-<name>.pdf",
        ),
        (request(plain_text, "A Test Book"), "is not a PDF"),
        (request(right_file, "?!"), "book title \"?!\""),
    ];

    for (ingest, hint) in refused {
        let failure = checked(&cx, &ingest)
            .await
            .expect_err("a check refuses this file");

        assert_eq!(failure.kind, FailureKind::BadFile, "{failure:?}");
        assert!(failure.hint.contains(hint), "{failure:?} must say {hint}");
        let (progress, finished) = started(&cx, &ingest).await;
        assert!(progress.is_empty(), "nothing started: {progress:?}");
        let failure = finished.expect_err("a start refuses this file too");
        assert_eq!(failure.kind, FailureKind::BadFile, "{failure:?}");
    }
}

#[tokio::test]
async fn with_the_stores_down_the_check_and_the_start_fail_with_the_store_named_and_nothing_is_paid()
 {
    let folder = tempfile::tempdir().expect("a temporary folder should be made");
    let (cx, pages) = support::closed_with_stub_pages(&folder.path().join("content"));
    let wanted = request(sample_pdf(), "Option Volatility and Pricing");
    let stores = [FailureKind::FalkorDbDown, FailureKind::QdrantDown];

    let failure = checked(&cx, &wanted)
        .await
        .expect_err("a check needs a store");

    assert!(stores.contains(&failure.kind), "{failure:?}");
    let (progress, finished) = started(&cx, &wanted).await;
    assert!(progress.is_empty(), "nothing was paid for: {progress:?}");
    let failure = finished.expect_err("a start needs the stores");
    assert!(stores.contains(&failure.kind), "{failure:?}");
    assert_eq!(pages.calls(), [], "no page was converted");
}
