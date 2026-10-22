//! Checks that a new chapter or paper is converted, stored and labelled, that a second start finds
//! it ingested, and that a part of a book is stored under its book.

use gui::backend::live::{LiveContext, Services};
use gui::contract::{
    Catalogue, Category, ChapterState, Command, Document, DocumentName, Event, IngestOutcome,
    IngestProgress, IngestRequest, IngestStage, InsightsOutcome, NewMedia, PageView, Preflight,
    Usage,
};
use ocr::testing::sample_pdf;

use super::{REQUEST, SAMPLE_BOOK, checked, events_of, request, started};
use crate::support;

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

/// The stage turns to converting when the last of the seven pages of the sample is cut out, before
/// the first one is saved. The saved pages are counted up one at a time, whatever order they end
/// in, and each later stage counts the `items` of the document. What the last step used is what
/// the run used.
fn assert_progress_of_one_run(progress: &[IngestProgress], items: u32, used: &Usage) {
    let stages: Vec<IngestStage> = progress.iter().map(|progress| progress.stage).collect();
    let mut expected = vec![IngestStage::CheckingStored, IngestStage::OpeningPdf];
    expected.extend((0..7).map(|ready| IngestStage::PreparingPages {
        ready,
        saved_before: 0,
        pages: 7,
    }));
    expected.extend((0..=7).map(|saved| IngestStage::Converting { saved, pages: 7 }));
    expected.extend([
        IngestStage::WritingGraph,
        IngestStage::Embedding { items },
        IngestStage::Storing,
        IngestStage::WritingInsights,
    ]);
    expected.extend((1..=items).map(|done| IngestStage::ReadingConcepts { done, items }));
    expected.extend((1..=items).map(|done| IngestStage::LinkingConcepts { done, items }));
    assert_eq!(stages, expected);
    let shares: Vec<f32> = stages
        .iter()
        .filter(|stage| {
            matches!(
                stage,
                IngestStage::PreparingPages { .. } | IngestStage::Converting { .. }
            )
        })
        .map(|stage| stage.share_done().expect("a page step has a share"))
        .collect();
    assert!(
        shares.windows(2).all(|pair| pair[0] <= pair[1]),
        "the share never falls from the first page step to the last: {shares:?}"
    );
    assert!(
        shares.last().is_some_and(|last| (last - 1.0).abs() < 1e-6),
        "the share is 1 when the last page is saved: {shares:?}"
    );
    assert!(
        progress.windows(2).all(|pair| {
            let (before, after) = (&pair[0].spent, &pair[1].spent);
            before.tokens() <= after.tokens() && before.cost_usd <= after.cost_usd
        }),
        "the tokens and the cost never go down: {progress:?}"
    );
    assert_eq!(progress.last().map(|progress| &progress.spent), Some(used));
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

/// Page 1 of a stored document, read from its folder.
async fn first_page_of<S: Services>(cx: &LiveContext<S>, stored: &Document) -> PageView {
    let command = Command::LoadPage {
        request: REQUEST,
        doc: stored.id,
        page: 1,
        folder: stored.folder.clone(),
    };
    let events = events_of(cx, command).await;
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
    result.clone().expect("page 1 should be read")
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
        pages: Some(7),
        state: Some(ChapterState::New),
        blockers: Vec::new(),
    };
    assert_eq!(before, Ok(new));

    let (progress, finished) = started(&cx, &wanted).await;

    let Ok(IngestOutcome::Ingested(report)) = finished else {
        panic!("expected the chapter to be ingested: {finished:#?}");
    };
    assert_eq!(report.pages, 7);
    assert_eq!(report.insights, Some(InsightsOutcome::Written));
    let counts = report.items;
    let items = counts.total();
    assert!(items > 0, "{report:#?}");
    let used: Vec<(&str, bool)> = (report.usage.models.iter())
        .map(|model| (model.model.as_str(), model.estimated))
        .collect();
    assert_eq!(
        used,
        [
            ("gemini-embedding-2", true),
            ("stand-in", false),
            ("stub-copier", false),
            ("stub-tagger", false),
            ("stub-transcriber", false)
        ]
    );
    assert!(report.usage.cost_usd.is_some());
    assert_progress_of_one_run(&progress, items as u32, &report.usage);
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

    let stages: Vec<IngestStage> = progress.iter().map(|progress| progress.stage).collect();
    assert_eq!(
        stages,
        [IngestStage::CheckingStored],
        "nothing is converted: {progress:?}"
    );
    let Ok(IngestOutcome::AlreadyIngested {
        doc,
        items: stored,
        insights,
        usage,
    }) = again
    else {
        panic!("expected the second start to find it ingested: {again:#?}");
    };
    assert_eq!(
        (doc, stored, insights),
        (report.doc, items, Some(InsightsOutcome::AlreadySaved))
    );
    assert!(usage.models.is_empty(), "nothing was asked: {usage:?}");
    assert_eq!(pages.calls(), calls_so_far, "no page was converted again");

    // A chapter stored before insights existed has no file: a start writes only that file.
    let folder = cx.config().content_folder.join(support::SAMPLE_PAGES);
    let file = folder.join("insights.json");
    std::fs::remove_file(&file).expect("the first start should have saved the insights");
    let without = checked(&cx, &wanted).await.unwrap();
    let (progress, writing) = started(&cx, &wanted).await;

    let missing = ChapterState::IngestedWithoutInsights { items };
    assert_eq!(without.state, Some(missing));
    assert!(without.blockers.is_empty(), "{without:?}");
    let stages: Vec<IngestStage> = progress.iter().map(|progress| progress.stage).collect();
    let writes_only_the_file = [IngestStage::CheckingStored, IngestStage::WritingInsights];
    assert_eq!(stages, writes_only_the_file);
    let Ok(IngestOutcome::AlreadyIngested {
        doc,
        items: stored,
        insights,
        usage,
    }) = writing
    else {
        panic!("expected the third start to find it ingested: {writing:#?}");
    };
    assert_eq!(
        (doc, stored, insights),
        (report.doc, items, Some(InsightsOutcome::Written))
    );
    let models: Vec<&str> = usage
        .models
        .iter()
        .map(|model| model.model.as_str())
        .collect();
    assert_eq!(models, ["stand-in"], "the insights alone were asked for");
    assert_eq!(pages.calls(), calls_so_far, "no page was converted again");
    assert!(file.is_file(), "the insights are written again");
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
        pages: Some(7),
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

    let page = first_page_of(&cx, stored).await;

    assert_eq!(page.chapter, None, "a paper has no chapter");
    assert_eq!(
        page.document_title.as_deref(),
        Some(paper),
        "the page names the paper by its title"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p gui --test integration -- --ignored ingest::"]
async fn a_part_of_a_book_is_ingested_with_its_book_in_its_stored_title() {
    let (cx, _pages) = support::context_and_pages("ingest-part");
    let media = NewMedia {
        title: SAMPLE_BOOK.to_owned(),
        category: Category::Book,
        authors: Vec::new(),
        tags: Vec::new(),
    };
    saved(&cx, media).await;
    // The sample pages under a name that is not a chapter's.
    let folder = tempfile::tempdir().expect("a temporary folder should be made");
    let index = folder.path().join("index.pdf");
    std::fs::copy(sample_pdf(), &index).expect("the sample pdf should be copied");
    let wanted = IngestRequest {
        pdf: index,
        media: SAMPLE_BOOK.to_owned(),
        category: Category::Book,
        name: DocumentName::Title("Index".to_owned()),
        tags: Vec::new(),
    };

    let before = checked(&cx, &wanted).await;

    let new = Preflight {
        name: DocumentName::Title("Index".to_owned()),
        pages: Some(7),
        state: Some(ChapterState::New),
        blockers: Vec::new(),
    };
    assert_eq!(before, Ok(new));

    let (_, finished) = started(&cx, &wanted).await;

    let Ok(IngestOutcome::Ingested(report)) = finished else {
        panic!("expected the part to be ingested: {finished:#?}");
    };
    assert_eq!(report.title, "Option Volatility and Pricing, Index");
    assert_eq!(report.insights, Some(InsightsOutcome::NoneForBookPart));
    let catalogue = catalogue_of(&cx).await;
    let book = catalogue
        .media_of(report.doc)
        .expect("the document is in a media");
    assert_eq!(book.title.as_deref(), Some(SAMPLE_BOOK));
    let [stored] = book.documents.as_slice() else {
        panic!("expected one document of the book: {book:#?}");
    };
    assert_eq!(
        stored.title, "Index",
        "the book lists its part by its own title"
    );
    assert_eq!(stored.chapter, None, "a part has no chapter");
    let stored_folder = stored.folder.as_deref().expect("the part has a folder");
    assert!(
        stored_folder.ends_with("option-volatility-and-pricing/index"),
        "{stored_folder:?}"
    );

    let page = first_page_of(&cx, stored).await;

    assert_eq!(page.chapter, None, "a part has no chapter");
    assert_eq!(
        page.document_title.as_deref(),
        Some("Index"),
        "the page names the part by its own title"
    );
}
