//! The lines that `rag-ingest` prints on standard error while a run works.

use ocr::PageProgress;
use rag_core::UsageTally;
use rag_ingestion::{IngestStep, usage_of};

/// The steps go to standard error, so standard output holds only the summary at the end, which
/// tells what the whole run used.
pub(super) fn print_step(step: IngestStep, _spent: &UsageTally) {
    if let Some(line) = progress_line(&step) {
        eprintln!("{line}");
    }
}

/// One line for each step before the pages and for each page cut out or converted, and one for
/// each later stage when it starts, never one for each item.
fn progress_line(step: &IngestStep) -> Option<String> {
    match step {
        IngestStep::CheckingStored => Some("checking what is already stored".to_owned()),
        IngestStep::OpeningPdf => Some("opening the pdf".to_owned()),
        IngestStep::Converting(PageProgress::Pages { total, done_before }) => Some(format!(
            "converting {} of {total} pages ({done_before} already done)",
            total - done_before
        )),
        IngestStep::Converting(PageProgress::PageCut { position }) => {
            Some(format!("page {position} cut out"))
        }
        IngestStep::Converting(PageProgress::PageDone { position, calls }) => {
            Some(match usage_of(calls).cost_usd() {
                Some(cost) => format!("page {position} converted (${cost:.2})"),
                None => format!("page {position} converted (cost unknown)"),
            })
        }
        IngestStep::Converting(PageProgress::PageFailed { position }) => {
            Some(format!("page {position} failed"))
        }
        IngestStep::WritingGraph => Some("writing the graph".to_owned()),
        IngestStep::Embedding { items } => Some(format!("embedding {items} items")),
        IngestStep::Storing => Some("storing the items".to_owned()),
        IngestStep::WritingArtifacts { done, total } => {
            Some(format!("writing artifact metadata: {done}/{total}"))
        }
        IngestStep::WritingInsights => {
            Some("writing the insights, takeaways and cheat sheet".to_owned())
        }
        IngestStep::ReadingConcepts { done: 1, total } => {
            Some(format!("reading the concepts of {total} items"))
        }
        IngestStep::LinkingConcepts { done: 1, total } => {
            Some(format!("linking the concepts of {total} items"))
        }
        IngestStep::ReadingConcepts { .. } | IngestStep::LinkingConcepts { .. } => None,
    }
}
