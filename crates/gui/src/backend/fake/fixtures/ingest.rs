//! The steps of an ingest and its report.

use std::path::PathBuf;

use uuid::Uuid;

use super::library::document_title;
use crate::contract::{
    Category, ChapterState, DocId, DocumentName, IngestReport, IngestRequest, ItemCounts,
    PageToCheck, Preflight,
};

/// The chapter that the two ingest scenes check as they open: the form as the page draws it, so
/// the page leaves it alone.
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

/// The panel sends a book chapter only when its file name is one, so the name is taken as it
/// comes.
pub(in crate::backend::fake) fn preflight(request: &IngestRequest) -> Preflight {
    Preflight {
        name: request.name.clone(),
        pages: None,
        state: Some(ChapterState::New),
        blockers: Vec::new(),
    }
}

pub(in crate::backend::fake) fn report(request: &IngestRequest) -> IngestReport {
    IngestReport {
        doc: DocId(Uuid::from_u128(9)),
        title: document_title(&request.media, &request.name),
        pages: 12,
        items: ItemCounts {
            chunks: 31,
            formulas: 8,
            figures: 3,
            tables: 2,
        },
        concepts_created: 14,
        concepts_linked: 9,
        skipped_items: 1,
        cost_usd: Some(1.84),
        pages_to_check: vec![PageToCheck {
            page: 7,
            reasons: vec!["A figure may be cut short.".to_owned()],
        }],
    }
}
