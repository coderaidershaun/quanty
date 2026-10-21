//! Asks the model once for the insights, the takeaways and the cheat sheet of a whole document
//! and saves them in its folder. A failed answer is told in the outcome and is never an error of
//! the ingest, because the items are stored before this step.

mod question;
mod stored;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ocr::content::INSIGHTS_FILE;
use ocr::{ChapterInsights, read_chapter};
use rag_core::{DocId, Llm, Question, UsageTally, error_chain};

use question::{DocumentText, Notes, PROMPT_VERSION, SCHEMA, SYSTEM_PROMPT};

pub use stored::{ChapterChoice, StoredChapter, StoredChaptersError, stored_chapters};

use super::items::{Item, chapter_items};
use super::summary::{IngestStep, Meter};

/// The stand-in model finds the question about a whole document by this prompt.
#[cfg(feature = "testing")]
pub(crate) use question::SYSTEM_PROMPT as INSIGHTS_PROMPT;

/// A question that fails is asked once more before it counts as failed.
const TRIES_FOR_ONE_QUESTION: usize = 2;

/// Writes the notes on a whole document with the language model it is given, the agent of the
/// Ingest choice in the programs.
pub struct InsightsWriter<L> {
    llm: L,
}

/// What became of the insights of one document, and what asking for them used.
#[derive(Debug, Clone, PartialEq)]
pub struct InsightsSummary {
    pub outcome: InsightsOutcome,
    /// What the questions used, a reply that was refused included.
    pub usage: UsageTally,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsightsOutcome {
    Written {
        file: PathBuf,
    },
    /// The file was there and could be read, so no question was asked.
    AlreadySaved {
        file: PathBuf,
    },
    /// Nothing was saved, and this is why.
    NotWritten {
        reason: String,
    },
    /// A part of a book that is not a chapter, such as its index, gets no insights, so no
    /// question was asked and no file was written.
    NoneForBookPart,
}

impl InsightsOutcome {
    /// Whether the run went to write the file, and so may have asked the model: true for a file
    /// that was written and for one that could not be. False when it was saved already, and when
    /// the document is one that gets none.
    pub fn tried_to_write(&self) -> bool {
        matches!(
            self,
            InsightsOutcome::Written { .. } | InsightsOutcome::NotWritten { .. }
        )
    }

    /// The line that a summary prints for the insights. `doc_id` is the document the insights
    /// belong to, so that a line that says they were not written can name the command that
    /// writes them.
    pub fn line(&self, doc_id: DocId) -> String {
        match self {
            InsightsOutcome::Written { file } => {
                format!("insights: written to {}", file.display())
            }
            InsightsOutcome::AlreadySaved { file } => format!(
                "insights: already saved in {}, so nothing was asked",
                file.display()
            ),
            InsightsOutcome::NotWritten { reason } => format!(
                "insights: not written: {reason}; the items are stored, so run `rag-ingest insights --document {doc_id}` to write them"
            ),
            InsightsOutcome::NoneForBookPart => {
                "insights: none, because a part of a book that is not a chapter gets none"
                    .to_owned()
            }
        }
    }
}

impl InsightsSummary {
    /// For a part of a book that is not a chapter: nothing was asked and nothing was used.
    pub(crate) fn none_for_book_part() -> InsightsSummary {
        InsightsSummary {
            outcome: InsightsOutcome::NoneForBookPart,
            usage: UsageTally::default(),
        }
    }

    fn not_written(reason: String) -> InsightsSummary {
        InsightsSummary {
            outcome: InsightsOutcome::NotWritten { reason },
            usage: UsageTally::default(),
        }
    }
}

/// What the tries at the one question came to.
enum Tries {
    Answered(Notes),
    /// Every try failed, and this is why the last one did.
    Failed(String),
}

impl<L: Llm> InsightsWriter<L> {
    pub fn new(llm: L) -> Self {
        InsightsWriter { llm }
    }

    /// Reads the converted chapter in the folder and writes its insights unless they are saved
    /// already. It is for a chapter that is stored already, and it never fails: the outcome says
    /// why nothing was written.
    pub async fn write_missing(&self, chapter_folder: &Path) -> InsightsSummary {
        let mut unheard = |_: IngestStep, _: &UsageTally| {};
        let mut meter = Meter::new(&mut unheard, UsageTally::default());
        self.write_missing_reporting(chapter_folder, &mut meter)
            .await
    }

    /// [`InsightsWriter::write_missing`], telling [`IngestStep::WritingInsights`] to the meter
    /// just before the question, and adding what each reply used to it.
    pub(crate) async fn write_missing_reporting(
        &self,
        chapter_folder: &Path,
        meter: &mut Meter<'_>,
    ) -> InsightsSummary {
        if let Some(saved) = saved_in(chapter_folder) {
            return saved;
        }
        let items = match read_chapter(chapter_folder) {
            Ok(chapter) => chapter_items(&chapter),
            Err(error) => return InsightsSummary::not_written(error_chain(&error)),
        };
        self.ask_and_save(chapter_folder, &items, meter).await
    }

    /// For an ingest that has the items already. The step is told only when the model will be
    /// asked.
    pub(crate) async fn write_missing_for_items(
        &self,
        chapter_folder: &Path,
        items: &[Item],
        meter: &mut Meter<'_>,
    ) -> InsightsSummary {
        if let Some(saved) = saved_in(chapter_folder) {
            return saved;
        }
        self.ask_and_save(chapter_folder, items, meter).await
    }

    async fn ask_and_save(
        &self,
        chapter_folder: &Path,
        items: &[Item],
        meter: &mut Meter<'_>,
    ) -> InsightsSummary {
        let text = match DocumentText::of(items) {
            Ok(text) => text,
            Err(error) => return InsightsSummary::not_written(error_chain(&error)),
        };
        meter.step(IngestStep::WritingInsights);
        let question = Question {
            system_prompt: SYSTEM_PROMPT,
            schema: SCHEMA,
            input: &text.text,
        };
        let mut usage = UsageTally::default();
        let outcome = match self.ask(question, &text.pages, meter, &mut usage).await {
            Tries::Answered(notes) => self.save(notes, chapter_folder),
            Tries::Failed(reason) => InsightsOutcome::NotWritten { reason },
        };
        InsightsSummary { outcome, usage }
    }

    /// An error that would fail every question ends the tries at once. Any other error, and a
    /// reply that does not pass the checks, is tried once more.
    async fn ask(
        &self,
        question: Question<'_>,
        shown: &BTreeSet<u32>,
        meter: &mut Meter<'_>,
        usage: &mut UsageTally,
    ) -> Tries {
        let mut reason = String::new();
        for _ in 0..TRIES_FOR_ONE_QUESTION {
            let reply = match self.llm.ask_long(question).await {
                Ok(reply) => reply,
                Err(error) if error.stops_the_run() => return Tries::Failed(error_chain(&error)),
                Err(error) => {
                    reason = error_chain(&error);
                    continue;
                }
            };
            usage.add_all(&reply.usage);
            meter.spend(&reply.usage);
            match Notes::from_value(&reply.value, shown) {
                Ok(notes) => return Tries::Answered(notes),
                Err(error) => reason = error_chain(&error),
            }
        }
        Tries::Failed(reason)
    }

    fn save(&self, notes: Notes, chapter_folder: &Path) -> InsightsOutcome {
        let insights = ChapterInsights {
            model: self.llm.model().to_owned(),
            prompt_version: PROMPT_VERSION.to_owned(),
            insights: notes.insights,
            takeaways: notes.takeaways,
            cheat_sheet: notes.cheat_sheet,
        };
        match insights.write(chapter_folder) {
            Ok(()) => InsightsOutcome::Written {
                file: chapter_folder.join(INSIGHTS_FILE),
            },
            Err(error) => InsightsOutcome::NotWritten {
                reason: error_chain(&error),
            },
        }
    }
}

/// A file that cannot be read counts as none, so it is asked for again and written over.
fn saved_in(chapter_folder: &Path) -> Option<InsightsSummary> {
    let saved = matches!(ChapterInsights::read(chapter_folder), Ok(Some(_)));
    saved.then(|| InsightsSummary {
        outcome: InsightsOutcome::AlreadySaved {
            file: chapter_folder.join(INSIGHTS_FILE),
        },
        usage: UsageTally::default(),
    })
}
