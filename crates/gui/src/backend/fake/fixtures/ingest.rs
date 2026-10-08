//! The steps of an ingest and its report.

use std::path::PathBuf;

use uuid::Uuid;

use super::library::document_title;
use crate::contract::{
    Category, ChapterState, DocId, DocumentName, IngestProgress, IngestReport, IngestRequest,
    IngestStage, ItemCounts, PageToCheck, Preflight,
};

const PAGES: u32 = 12;
const COST_OF_A_PAGE: f64 = 0.14;
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
    let at = |stage, done, total, cost_usd| IngestProgress {
        stage,
        done,
        total,
        pages_failed: 0,
        cost_usd,
    };
    let every_page = cost_of_pages(PAGES);
    let mut steps = vec![at(IngestStage::PreparingPages, None, Some(PAGES), 0.0)];
    steps.extend((1..=PAGES).map(|page| {
        at(
            IngestStage::Converting,
            Some(page),
            Some(PAGES),
            cost_of_pages(page),
        )
    }));
    steps.extend([
        at(IngestStage::WritingGraph, None, None, every_page),
        at(IngestStage::Embedding, None, Some(ITEMS), every_page),
        at(IngestStage::Storing, None, None, every_page),
    ]);
    for stage in [IngestStage::ReadingConcepts, IngestStage::LinkingConcepts] {
        steps.extend(
            (1..=4).map(|quarter| at(stage, Some(ITEMS / 4 * quarter), Some(ITEMS), every_page)),
        );
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

fn cost_of_pages(pages: u32) -> f64 {
    COST_OF_A_PAGE * f64::from(pages)
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
        cost_usd: Some(cost_of_pages(PAGES)),
        pages_to_check: vec![PageToCheck {
            page: 7,
            reasons: vec!["A figure may be cut short.".to_owned()],
        }],
    }
}
