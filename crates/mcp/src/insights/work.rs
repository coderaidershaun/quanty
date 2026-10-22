//! The work of one insights job: the chosen documents one after another, each with the insights it
//! needs written and saved, and a line for the report after each.

use std::path::PathBuf;

use ocr::content::INSIGHTS_FILE;
use rag_core::{Llm, UsageTally};
use rag_ingestion::{InsightsNeed, InsightsOutcome, InsightsWriter, StoredChapter};
use tokio::sync::watch;

use super::report::{DocumentLine, InsightsReport, Outcome};

/// One insights job, with everything it needs to run on a task of its own.
pub(super) struct Work<L> {
    pub(super) chapters: Vec<StoredChapter>,
    /// Asks the model for the insights that are missing. A job that writes nothing has none: a dry
    /// run asks no model, and a job with nothing missing never gets as far as a question.
    pub(super) writer: Option<InsightsWriter<L>>,
    /// Named in the line of a document that has no converted chapter.
    pub(super) content_folder: PathBuf,
    /// Where each document is written. The end of the report is written by the task that waits
    /// for this one, not here.
    pub(super) report: watch::Sender<InsightsReport>,
}

impl<L: Llm> Work<L> {
    pub(super) async fn run(self) {
        let mut usage = UsageTally::default();
        for chapter in &self.chapters {
            let line = self.handle(chapter, &mut usage).await;
            self.report
                .send_modify(|report| report.handled(line, &usage));
        }
    }

    async fn handle(&self, chapter: &StoredChapter, usage: &mut UsageTally) -> DocumentLine {
        let line = |outcome| DocumentLine::new(chapter, outcome);
        match chapter.need() {
            InsightsNeed::BookPart => line(Outcome::NoneForBookPart),
            InsightsNeed::NoChapter => line(Outcome::NoConvertedChapter).with_reason(format!(
                "no converted chapter of this document is under {}; check that the server reads the content folder that the document was ingested from",
                self.content_folder.display()
            )),
            InsightsNeed::AlreadySaved => line(Outcome::AlreadySaved),
            InsightsNeed::Write(folder) => {
                let Some(writer) = &self.writer else {
                    return line(Outcome::WouldWrite).with_file(&folder.join(INSIGHTS_FILE));
                };
                self.report
                    .send_modify(|report| report.asking(&chapter.title));
                let summary = writer.write_missing(folder).await;
                usage.add_all(&summary.usage);
                line_of(chapter, summary.outcome)
            }
        }
    }
}

fn line_of(chapter: &StoredChapter, outcome: InsightsOutcome) -> DocumentLine {
    match outcome {
        InsightsOutcome::Written { file } => {
            DocumentLine::new(chapter, Outcome::Written).with_file(&file)
        }
        InsightsOutcome::AlreadySaved { .. } => DocumentLine::new(chapter, Outcome::AlreadySaved),
        InsightsOutcome::NotWritten { reason } => {
            DocumentLine::new(chapter, Outcome::NotWritten).with_reason(reason)
        }
        InsightsOutcome::NoneForBookPart => DocumentLine::new(chapter, Outcome::NoneForBookPart),
    }
}
