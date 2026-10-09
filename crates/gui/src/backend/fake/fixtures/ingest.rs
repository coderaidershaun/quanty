//! The steps of an ingest and its report.

use std::path::PathBuf;

use uuid::Uuid;

use super::library::document_title;
use crate::contract::{
    Category, ChapterState, DocId, DocumentName, IngestProgress, IngestReport, IngestRequest,
    IngestStage, ItemCounts, ModelTokens, PageToCheck, Preflight, Usage,
};

const PAGES: u32 = 12;
// SMELL: the dollars of this play are made up and do not follow the price table, so they are
// about twice what its tokens would cost.
const COST_OF_A_PAGE: f64 = 0.14;
/// What Sonnet reads and what it writes to convert one page.
const TOKENS_OF_A_PAGE: (u64, u64) = (12_000, 4_000);
const ITEM_COUNTS: ItemCounts = ItemCounts {
    chunks: 31,
    formulas: 8,
    figures: 3,
    tables: 2,
};
const ITEMS: u32 =
    (ITEM_COUNTS.chunks + ITEM_COUNTS.formulas + ITEM_COUNTS.figures + ITEM_COUNTS.tables) as u32;
/// The page at which the play of a scene whose ingest never ends stops for ever.
const STALLED_AT_PAGE: u32 = 3;

/// The chapter that the three ingest scenes check as they open: the form as the page draws it,
/// so the page leaves it alone.
pub(in crate::backend::fake) fn request() -> IngestRequest {
    IngestRequest {
        pdf: PathBuf::from("/books/option-volatility/chapter-3-greeks.pdf"),
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
pub(in crate::backend::fake) fn preflight(request: &IngestRequest) -> Preflight {
    Preflight {
        name: request.name.clone(),
        pages: Some(PAGES),
        state: Some(ChapterState::New),
        blockers: Vec::new(),
    }
}

/// The play of one ingest of the chapter: the count of its pages, each page, the later stages,
/// then the concepts read and linked in four steps each.
pub(in crate::backend::fake) fn steps() -> Vec<IngestProgress> {
    let at = |stage, done, total, spent| IngestProgress {
        stage,
        done,
        total,
        pages_failed: 0,
        spent,
    };
    let every_page = spent_on_pages(PAGES);
    let mut steps = vec![at(
        IngestStage::PreparingPages,
        None,
        Some(PAGES),
        Usage::default(),
    )];
    steps.extend((1..=PAGES).map(|page| {
        at(
            IngestStage::Converting,
            Some(page),
            Some(PAGES),
            spent_on_pages(page),
        )
    }));
    steps.extend([
        at(IngestStage::WritingGraph, None, None, every_page.clone()),
        at(
            IngestStage::Embedding,
            None,
            Some(ITEMS),
            every_page.clone(),
        ),
        at(IngestStage::Storing, None, None, every_page.clone()),
    ]);
    for stage in [IngestStage::ReadingConcepts, IngestStage::LinkingConcepts] {
        steps.extend((1..=4).map(|quarter| {
            let done = ITEMS / 4 * quarter;
            at(stage, Some(done), Some(ITEMS), every_page.clone())
        }));
    }
    steps
}

/// The play up to and including the step that converts page [`STALLED_AT_PAGE`].
pub(in crate::backend::fake) fn steps_to_the_stall() -> Vec<IngestProgress> {
    let mut steps = steps();
    // The play opens with the count of the pages, then has one step for each page.
    steps.truncate(1 + STALLED_AT_PAGE as usize);
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

pub(in crate::backend::fake) fn report(request: &IngestRequest) -> IngestReport {
    IngestReport {
        doc: DocId(Uuid::from_u128(9)),
        title: document_title(&request.media, &request.name),
        pages: PAGES,
        items: ITEM_COUNTS,
        concepts_created: 14,
        concepts_linked: 9,
        skipped_items: 1,
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
    }
}
