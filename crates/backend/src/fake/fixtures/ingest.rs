//! The steps of an ingest and its report.

use std::io::Read;
use std::path::Path;
use uuid::Uuid;

use super::library::document_title;
use contract::{
    Category, ChapterState, DocId, DocumentName, IngestProgress, IngestReport, IngestRequest,
    IngestStage, InsightsOutcome, ItemCounts, ModelTokens, PageToCheck, Preflight, Usage,
};

const PAGES: u32 = 12;
// SMELL: the dollars of this play are made up, about twice what the price table gives for its
// tokens, because the fake may not name the crate that holds the table.
const COST_OF_A_PAGE: f64 = 0.14;
/// What Sonnet reads and what it writes to convert one page.
const TOKENS_OF_A_PAGE: (u64, u64) = (12_000, 4_000);
const ITEM_COUNTS: ItemCounts = ItemCounts {
    chunks: 31,
    formulas: 8,
    figures: 3,
    tables: 2,
    code: 0,
};
const ITEMS: u32 = ITEM_COUNTS.total() as u32;
/// The page at which the play of a scene whose ingest never ends stops for ever.
const STALLED_AT_PAGE: u32 = 3;

/// The chapter that the four ingest scenes check as they open: the form as the page draws it,
/// so the page leaves it alone.
pub(in crate::fake) fn request() -> IngestRequest {
    IngestRequest {
        pdf: contract::UploadId::default(),
        file_name: "chapter-3-greeks.pdf".into(),
        media: "Option Volatility and Pricing".to_owned(),
        category: Category::Book,
        name: DocumentName::Chapter {
            number: 3,
            name: "Greeks".to_owned(),
        },
        tags: vec!["greeks".to_owned()],
    }
}

/// The panel sends a name only once it is complete, so the name is taken as it comes.
pub(in crate::fake) fn preflight(
    request: &IngestRequest,
    samples: &Path,
) -> Result<Preflight, contract::Failure> {
    let pdf = samples
        .parent()
        .unwrap_or(samples)
        .join("chapter-1-sample-pages.pdf");
    let invalid = |detail: String| {
        contract::Failure::new(contract::FailureKind::BadFile, detail)
        .with_hint("The sample PDF cannot be read. Restore samples/chapter-1-sample-pages.pdf, then try again.")
    };
    let mut header = [0; 5];
    std::fs::File::open(&pdf)
        .and_then(|mut file| file.read_exact(&mut header))
        .map_err(|error| invalid(format!("could not read {}: {error}", pdf.display())))?;
    if header != *b"%PDF-" {
        return Err(invalid(format!("{} is not a PDF", pdf.display())));
    }
    Ok(Preflight {
        name: request.name.clone(),
        pages: Some(PAGES),
        state: Some(ChapterState::New),
        blockers: Vec::new(),
    })
}

/// The play of one ingest of the chapter: the stages that a real run tells, in the same order.
/// Only the concepts differ: they are read and linked in four steps each, not one for each item.
pub(in crate::fake) fn steps() -> Vec<IngestProgress> {
    let at = |stage, spent| IngestProgress { stage, spent };
    let every_page = spent_on_pages(PAGES);
    let mut steps = vec![
        at(IngestStage::CheckingStored, Usage::default()),
        at(IngestStage::OpeningPdf, Usage::default()),
    ];
    steps.extend((0..PAGES).map(|ready| {
        let preparing = IngestStage::PreparingPages {
            ready,
            saved_before: 0,
            pages: PAGES,
        };
        at(preparing, Usage::default())
    }));
    steps.extend((0..=PAGES).map(|saved| {
        let converting = IngestStage::Converting {
            saved,
            pages: PAGES,
        };
        at(converting, spent_on_pages(saved))
    }));
    steps.extend([
        at(
            IngestStage::WritingArtifacts { done: 0, items: 8 },
            every_page.clone(),
        ),
        at(
            IngestStage::WritingArtifacts { done: 8, items: 8 },
            every_page.clone(),
        ),
        at(IngestStage::WritingGraph, every_page.clone()),
        at(IngestStage::Embedding { items: ITEMS }, every_page.clone()),
        at(IngestStage::Storing, every_page.clone()),
        at(IngestStage::WritingInsights, every_page.clone()),
    ]);
    let quarters = || (1..=4).map(|quarter| ITEMS / 4 * quarter);
    steps.extend(quarters().map(|done| {
        let reading = IngestStage::ReadingConcepts { done, items: ITEMS };
        at(reading, every_page.clone())
    }));
    steps.extend(quarters().map(|done| {
        let linking = IngestStage::LinkingConcepts { done, items: ITEMS };
        at(linking, every_page.clone())
    }));
    steps
}

/// The play up to and including the step that converts page [`STALLED_AT_PAGE`].
pub(in crate::fake) fn steps_to_the_stall() -> Vec<IngestProgress> {
    let stall = IngestStage::Converting {
        saved: STALLED_AT_PAGE,
        pages: PAGES,
    };
    let mut steps = steps();
    let stalled_at = steps
        .iter()
        .position(|step| step.stage == stall)
        .expect("the play converts every page");
    steps.truncate(stalled_at + 1);
    steps
}

/// What converting the first `pages` pages used: Sonnet alone.
fn spent_on_pages(pages: u32) -> Usage {
    let (input, output) = TOKENS_OF_A_PAGE;
    let pages_read = u64::from(pages);
    Usage {
        models: vec![model(
            "claude-sonnet-5-5",
            input * pages_read,
            output * pages_read,
        )],
        cost_usd: Some(COST_OF_A_PAGE * f64::from(pages)),
    }
}

fn model(name: &str, input: u64, output: u64) -> ModelTokens {
    ModelTokens {
        model: name.to_owned(),
        input,
        output,
        ..ModelTokens::default()
    }
}

/// The title the ingest stores: a part of a book is stored under its book, as the live ingest
/// stores it.
// SMELL: the converter keeps its own copy of the rule that joins a book's title and a part's
// title, and the fake may not name that crate, so a change to one must be made in both.
fn stored_title(request: &IngestRequest) -> String {
    match (request.category, &request.name) {
        (Category::Book, DocumentName::Title(part)) => format!("{}, {}", request.media, part),
        _ => document_title(&request.media, &request.name),
    }
}

/// The report of the play. The insights are written, unless the document is one that gets none.
pub(in crate::fake) fn report(request: &IngestRequest) -> IngestReport {
    let insights = if request.name.gets_insights(Some(request.category)) {
        InsightsOutcome::Written
    } else {
        InsightsOutcome::NoneForBookPart
    };
    IngestReport {
        doc: DocId(Uuid::from_u128(9)),
        title: stored_title(request),
        pages: PAGES,
        items: ITEM_COUNTS,
        concepts_created: 14,
        concepts_linked: 9,
        skipped_items: 1,
        artifacts: 8,
        usage: Usage {
            models: vec![
                model("claude-haiku-5-5", 44_000, 4_400),
                model("claude-sonnet-5-5", 144_000, 48_000),
                ModelTokens {
                    estimated: true,
                    ..model("gemini-embedding-2", 9_000, 0)
                },
            ],
            cost_usd: Some(1.75),
        },
        pages_to_check: vec![PageToCheck {
            page: 7,
            reasons: vec!["A figure may be cut short.".to_owned()],
        }],
        insights: Some(insights),
    }
}

/// The same report for a run whose insights could not be written, with the items stored.
pub(in crate::fake) fn report_with_insights_not_written(request: &IngestRequest) -> IngestReport {
    IngestReport {
        insights: Some(InsightsOutcome::NotWritten {
            reason: "claude reached its usage limit".to_owned(),
        }),
        ..report(request)
    }
}
