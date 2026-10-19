//! Asks models for the synthetic texts of a dataset, written from the sections the plan chose, and
//! keeps a text only when a judge finds it faithful to its source.

pub mod asker;
pub mod role;

mod draft;
mod entigraph;
mod judge;
mod rewrite;

pub use entigraph::ENTIGRAPH;
pub use judge::JUDGE;
pub use rewrite::{EXERCISE, NOTATION, REPHRASE_LECTURE, REPHRASE_QA, REPHRASE_WIKI};

use futures_util::StreamExt;
use rag_core::{Llm, UsageTally};

use crate::layout::{DatasetFolder, WriteError, write_jsonl};
use crate::options::BuildOptions;
use crate::plan::GenerationPlan;
use crate::progress::{GenerationKind, Step, Written};
use crate::section::Section;

use asker::{AskError, Asker, few_at_a_time};
use draft::Outcome;
use role::Role;

/// Every role this stage asks. The manifest lists their versions.
pub(crate) const ROLES: [&Role; 7] = [
    &REPHRASE_WIKI,
    &REPHRASE_LECTURE,
    &REPHRASE_QA,
    &ENTIGRAPH,
    &EXERCISE,
    &NOTATION,
    &JUDGE,
];

#[derive(thiserror::Error, Debug)]
pub enum GenerateError {
    #[error(transparent)]
    Ask(#[from] AskError),

    #[error(transparent)]
    Write(#[from] WriteError),
}

/// Writes `cpt/synthetic.jsonl`: every text the plan asks for that a judge found faithful to its
/// source, in the order of the plan. The texts are written a few at a time, and `on_step` is told
/// after each one. The file is written once, at the end, so a run that stops writes no file; the
/// answers given before the stop are kept, and the next run does not ask for them again.
///
/// A text whose writer or judge gave no answer, or that the judge turned down, is left out and
/// counted in `rejected` by the role that wrote it.
///
/// # Errors
/// [`GenerateError::Ask`] when the model can answer no question or the kept answers cannot be
/// used, and [`GenerateError::Write`] when the file cannot be written.
pub async fn write_synthetic<L: Llm>(
    plan: &GenerationPlan,
    sections: &[Section],
    asker: &Asker<'_, L>,
    folder: &DatasetFolder,
    _options: &BuildOptions,
    on_step: &mut (dyn FnMut(Step, &UsageTally) + Send),
) -> Result<Written, GenerateError> {
    let drafts = draft::planned(plan, sections);
    let total = drafts.len();
    let step = |done| Step::Writing {
        kind: GenerationKind::Synthetic,
        done,
        total,
    };
    on_step(step(0), &asker.usage());

    // Every write is made here, before the stream, and none starts until the stream polls it. Do
    // not make them with a closure inside the stream: the compiler then cannot prove that this
    // future is `Send`, and no caller could spawn a build.
    let writes: Vec<_> = drafts.iter().map(|draft| draft.write(asker)).collect();
    let mut outcomes = few_at_a_time(writes);
    let mut records = Vec::new();
    let mut written = Written::default();
    let mut done = 0;
    while let Some(outcome) = outcomes.next().await {
        match outcome? {
            Outcome::Kept(record) => {
                *written
                    .by_kind
                    .entry(record.kind.as_str().to_owned())
                    .or_insert(0) += 1;
                records.push(*record);
            }
            Outcome::Rejected { role } => {
                *written.rejected.entry(role.id.to_owned()).or_insert(0) += 1;
            }
        }
        done += 1;
        on_step(step(done), &asker.usage());
    }
    write_jsonl(&folder.cpt_synthetic(), &records)?;
    Ok(written)
}
