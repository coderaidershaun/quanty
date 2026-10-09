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
        IngestStep::ReadingConcepts { done: 1, total } => {
            Some(format!("reading the concepts of {total} items"))
        }
        IngestStep::LinkingConcepts { done: 1, total } => {
            Some(format!("linking the concepts of {total} items"))
        }
        IngestStep::ReadingConcepts { .. } | IngestStep::LinkingConcepts { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use ocr::PageProgress;
    use ocr::convert::{CallTally, ModelCalls};
    use rag_ingestion::IngestStep;

    use super::progress_line;

    #[test]
    fn progress_lines_name_each_page_and_each_stage_once() {
        let pages = IngestStep::Converting(PageProgress::Pages {
            total: 12,
            done_before: 4,
        });
        // At the table's price of Sonnet 5.5 this is $0.12 in and $0.017 out, so $0.14. The
        // $0.50 that claude reported is not used, because the table has the model.
        let calls = CallTally {
            transcribe: 1,
            by_model: [(
                "claude-sonnet-5-5".to_owned(),
                ModelCalls {
                    input_tokens: 60_000,
                    output_tokens: 1_700,
                    cost_usd: 0.5,
                    ..ModelCalls::default()
                },
            )]
            .into(),
            ..CallTally::default()
        };
        let page = IngestStep::Converting(PageProgress::PageDone { position: 5, calls });
        let first_read = IngestStep::ReadingConcepts { done: 1, total: 44 };
        let second_read = IngestStep::ReadingConcepts { done: 2, total: 44 };

        assert_eq!(
            progress_line(&pages).as_deref(),
            Some("converting 8 of 12 pages (4 already done)")
        );
        assert_eq!(
            progress_line(&page).as_deref(),
            Some("page 5 converted ($0.14)")
        );
        assert_eq!(
            progress_line(&first_read).as_deref(),
            Some("reading the concepts of 44 items")
        );
        assert_eq!(progress_line(&second_read), None);
    }
}
