//! Writes the fine-tuning examples of a dataset: questions that models write from the sections,
//! the answers they give, and the checks that decide which answers are kept.

mod python;
mod questions;
mod records;
mod solve;
mod verify;

use rag_core::{Llm, UsageTally};

use crate::generate::asker::{AskError, Asker};
use crate::generate::role::Role;
use crate::layout::{DatasetFolder, WriteError, write_jsonl};
use crate::options::BuildOptions;
use crate::plan::GenerationPlan;
use crate::progress::{GenerationKind, Step, Written};
use crate::section::Section;

use questions::AskedQuestions;
use records::Records;

pub use python::ProgramError;
pub use questions::QUESTIONS;
pub use solve::{SOLVE_DIRECT, SOLVE_THINK};
pub use verify::JUDGE_ANSWER;

/// Every role this stage asks. The manifest lists their versions.
pub(crate) const ROLES: [&Role; 4] = [&QUESTIONS, &SOLVE_THINK, &SOLVE_DIRECT, &JUDGE_ANSWER];

/// How many sections are worked through together. The asker asks four questions at a time, so
/// four sections fill one round of questions, and the progress moves after every step.
const SECTIONS_PER_STEP: usize = 4;

#[derive(thiserror::Error, Debug)]
pub enum SftError {
    #[error(transparent)]
    Ask(#[from] AskError),

    #[error(transparent)]
    Write(#[from] WriteError),

    #[error(transparent)]
    Program(#[from] ProgramError),
}

/// Asks the model for questions on the sections the plan gives questions to, then solves and
/// checks each one. An example that passed goes to the training file or the held-out file, by the
/// side of its document, and one that failed goes to the rejected file. The files are written
/// whole at the end, so a run that stops leaves none.
///
/// # Errors
/// - [`SftError::Ask`] when the model can answer no question
/// - [`SftError::Program`] when a program that a model wrote cannot be written to a temporary
///   folder to run
/// - [`SftError::Write`] when a file cannot be written
pub async fn write_sft<L: Llm>(
    plan: &GenerationPlan,
    sections: &[Section],
    asker: &Asker<'_, L>,
    folder: &DatasetFolder,
    options: &BuildOptions,
    on_step: &mut (dyn FnMut(Step, &UsageTally) + Send),
) -> Result<Written, SftError> {
    let planned = questions::planned(plan, sections);
    let total = questions::count_of(&planned);
    let writing = |done| Step::Writing {
        kind: GenerationKind::Sft,
        done,
        total,
    };
    on_step(writing(0), &asker.usage());

    let needs_python = options.run_code && total > 0;
    let run_code = needs_python && python::is_usable().await;
    if needs_python && !run_code {
        tracing::warn!(
            "python3 cannot be run here, so the questions that ask for code are checked by the judge alone"
        );
    }
    let samples = options.samples.max(1);

    let mut records = Records::default();
    let mut done = 0;
    for step in planned.chunks(SECTIONS_PER_STEP) {
        let AskedQuestions { asked, unanswered } = questions::ask_questions(asker, step).await?;
        records.add_unanswered(unanswered);
        let solved = solve::solve(asker, &asked, samples).await?;
        let outcomes = verify::run_checks(&asked, solved, run_code).await?;
        let verified = verify::judge(asker, &asked, outcomes).await?;
        for (question, verified) in asked.iter().zip(verified) {
            records.add(question, verified);
        }
        done += questions::count_of(step);
        on_step(writing(done), &asker.usage());
    }

    write_jsonl(&folder.sft_train(), &records.train)?;
    write_jsonl(&folder.sft_heldout(), &records.heldout)?;
    write_jsonl(&folder.sft_rejected(), &records.rejected)?;
    Ok(records.written)
}
