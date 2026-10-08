//! Checks that the live backend checks and runs an ingest of a PDF of a saved media: a new
//! chapter or paper is converted, stored and labelled, and a store that is down fails the run with
//! nothing paid for.

use gui::backend::live::{LiveContext, Services};
use gui::backend::{Handler, Reply};
use gui::contract::{
    Catalogue, Category, ChapterState, Command, DocumentName, Event, Failure, FailureKind,
    IngestOutcome, IngestProgress, IngestRequest, IngestStage, NewMedia, Preflight, RequestId,
};
use ocr::testing::sample_pdf;

use crate::support;

const REQUEST: RequestId = RequestId(7);
const SAMPLE_BOOK: &str = "Option Volatility and Pricing";

fn request() -> IngestRequest {
    IngestRequest {
        pdf: sample_pdf(),
        media: SAMPLE_BOOK.to_owned(),
        category: Category::Book,
        name: DocumentName::Chapter {
            number: 1,
            name: "Sample Pages".to_owned(),
        },
        tags: Vec::new(),
    }
}

/// Saves the media the way the Ingest tab does before its first PDF.
async fn saved<S: Services>(cx: &LiveContext<S>, media: NewMedia) {
    let command = Command::SaveMedia {
        request: REQUEST,
        media,
    };
    let events = events_of(cx, command).await;
    assert!(
        matches!(
            events.as_slice(),
            [Event::MediaSaved {
                request: REQUEST,
                result: Ok(()),
            }]
        ),
        "expected the media to be saved: {events:#?}"
    );
}

async fn events_of<S: Services>(cx: &LiveContext<S>, command: Command) -> Vec<Event> {
    let (reply, events, _stop) = Reply::collecting();
    cx.serve(command, reply).await;
    events.try_iter().collect()
}

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
    let (cx, pages) = support::context_and_pages("ingest-run");
    let media = NewMedia {
        title: SAMPLE_BOOK.to_owned(),
        category: Category::Book,
        authors: vec![" Sheldon Natenberg ".to_owned()],
        tags: vec!["Options".to_owned()],
    };
    saved(&cx, media).await;
    let wanted = IngestRequest {
        tags: vec!["Greeks".to_owned(), "volatility".to_owned(), " ".to_owned()],
        ..request()
    };

    let before = checked(&cx, &wanted).await;

    let new = Preflight {
        name: wanted.name.clone(),
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
        .media_of(report.doc)
        .expect("the document is in a media");
    assert_eq!(book.title.as_deref(), Some(SAMPLE_BOOK));
    assert_eq!(
        (book.category, &book.authors, &book.tags),
        (
            Category::Book,
            &vec!["Sheldon Natenberg".to_owned()],
            &vec!["options".to_owned()]
        )
    );
    let stored = catalogue
        .document(report.doc)
        .expect("the document is stored");
    assert_eq!(stored.authors, ["Sheldon Natenberg"]);
    assert_eq!(stored.media_tags, ["options"]);
    assert_eq!(
        stored.tags,
        ["greeks", "volatility"],
        "the own tags of the PDF"
    );

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
async fn a_paper_pdf_with_a_plain_name_is_checked_and_ingested_under_its_saved_media() {
    let (cx, _pages) = support::context_and_pages("ingest-paper");
    let paper = "Hawkes Processes in Finance";
    let media = NewMedia {
        title: paper.to_owned(),
        category: Category::Paper,
        authors: vec!["A. Author".to_owned(), "B. Author".to_owned()],
        tags: vec!["hawkes".to_owned()],
    };
    saved(&cx, media).await;
    // The sample pages under a name that is not a chapter's.
    let folder = tempfile::tempdir().expect("a temporary folder should be made");
    let plain = folder.path().join("hawkes-notes.pdf");
    std::fs::copy(sample_pdf(), &plain).expect("the sample pdf should be copied");
    let wanted = IngestRequest {
        pdf: plain,
        media: paper.to_owned(),
        category: Category::Paper,
        name: DocumentName::Title(paper.to_owned()),
        tags: Vec::new(),
    };

    let before = checked(&cx, &wanted).await;

    let new = Preflight {
        name: DocumentName::Title(paper.to_owned()),
        pages: None,
        state: Some(ChapterState::New),
        blockers: Vec::new(),
    };
    assert_eq!(before, Ok(new));

    let (_, finished) = started(&cx, &wanted).await;

    let Ok(IngestOutcome::Ingested(report)) = finished else {
        panic!("expected the paper to be ingested: {finished:#?}");
    };
    assert_eq!(report.title, paper);
    let catalogue = catalogue_of(&cx).await;
    let media = catalogue
        .media_of(report.doc)
        .expect("the document is in a media");
    assert_eq!(media.title.as_deref(), Some(paper));
    assert_eq!(media.category, Category::Paper);
    let [stored] = media.documents.as_slice() else {
        panic!("expected one document of the paper: {media:#?}");
    };
    assert_eq!(stored.title, paper);
    assert_eq!(stored.chapter, None, "a paper has no chapter");
    assert_eq!(stored.authors, ["A. Author", "B. Author"]);
    assert_eq!(stored.media_tags, ["hawkes"]);
    assert_eq!(stored.pages, Some(7));

    let command = Command::LoadPage {
        request: REQUEST,
        doc: stored.id,
        page: 1,
        folder: stored.folder.clone(),
    };
    let events = events_of(&cx, command).await;

    let [
        Event::Page {
            request: REQUEST,
            result,
        },
        ..,
    ] = events.as_slice()
    else {
        panic!("expected the page first: {events:#?}");
    };
    let page = result.as_ref().expect("page 1 of the paper should be read");
    assert_eq!(page.chapter, None, "a paper has no chapter");
    assert_eq!(
        page.document_title.as_deref(),
        Some(paper),
        "the page names the paper by its title"
    );
}

#[tokio::test]
async fn with_the_stores_down_the_check_and_the_start_fail_with_the_store_named_and_nothing_is_paid()
 {
    let folder = tempfile::tempdir().expect("a temporary folder should be made");
    let (cx, pages) = support::closed_with_stub_pages(&folder.path().join("content"));
    let wanted = request();
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
