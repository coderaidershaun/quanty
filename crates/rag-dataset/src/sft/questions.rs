//! The questions that a model writes from the planned sections, how every later ask shows a
//! section, and how the asks and the files spell a task.

use std::collections::HashMap;

use rag_core::Llm;
use serde::{Deserialize, Serialize};

use crate::generate::asker::{Answer, AskError, Asker};
use crate::generate::role::Role;
use crate::plan::GenerationPlan;
use crate::sample::{SeedKind, Task};
use crate::section::Section;

/// Writes up to a number of questions from one section.
pub const QUESTIONS: Role = Role {
    id: "questions",
    version: 1,
    system_prompt: include_str!("prompts/questions.md"),
    schema: include_str!("prompts/questions.schema.json"),
};

/// One question as the model wrote it.
#[derive(Debug, Clone, Deserialize)]
pub(super) struct WrittenQuestion {
    pub(super) question: String,
    pub(super) task: Task,
    pub(super) difficulty: f64,
    pub(super) seed: WrittenSeed,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct WrittenSeed {
    pub(super) kind: SeedKind,
    pub(super) quote: String,
}

#[derive(Deserialize)]
struct WrittenQuestions {
    questions: Vec<WrittenQuestion>,
}

/// One question of a section. `number` counts from 1 within the section, in the order the model
/// wrote the questions.
pub(super) struct Asked<'a> {
    pub(super) section: &'a Section,
    pub(super) number: u32,
    pub(super) written: WrittenQuestion,
}

pub(super) struct AskedQuestions<'a> {
    /// In the order of the sections, then of the numbers.
    pub(super) asked: Vec<Asked<'a>>,
    /// The planned questions of the sections for which the model wrote none.
    pub(super) unanswered: usize,
}

/// The sections of the plan that have questions, each with how many. A section that the plan names
/// and the sections do not hold is skipped.
pub(super) fn planned<'a>(
    plan: &GenerationPlan,
    sections: &'a [Section],
) -> Vec<(&'a Section, u32)> {
    let by_id: HashMap<&str, &Section> = sections
        .iter()
        .map(|section| (section.id.as_str(), section))
        .collect();
    plan.sections
        .iter()
        .filter(|section_plan| section_plan.questions > 0)
        .filter_map(|section_plan| {
            let section = by_id.get(section_plan.section_id.as_str())?;
            Some((*section, section_plan.questions))
        })
        .collect()
}

/// How many questions the sections are planned to get.
pub(super) fn count_of(planned: &[(&Section, u32)]) -> usize {
    planned.iter().map(|(_, count)| *count as usize).sum()
}

/// Asks for the questions of all the given sections in one batch.
///
/// # Errors
/// The errors of [`Asker::ask_each`].
pub(super) async fn ask_questions<'a, L: Llm>(
    asker: &Asker<'_, L>,
    planned: &[(&'a Section, u32)],
) -> Result<AskedQuestions<'a>, AskError> {
    let inputs: Vec<String> = planned
        .iter()
        .map(|(section, count)| {
            format!("Write up to {count} questions.\n\n{}", material_of(section))
        })
        .collect();
    let answers = asker
        .ask_each::<WrittenQuestions>(&QUESTIONS, &inputs)
        .await?;
    let mut asked = Vec::new();
    let mut unanswered = 0;
    for ((section, count), answer) in planned.iter().zip(answers) {
        match answer {
            Answer::Given(reply) => {
                let questions = reply
                    .questions
                    .into_iter()
                    .filter(|written| !written.question.trim().is_empty())
                    .take(*count as usize);
                for (number, written) in (1..).zip(questions) {
                    asked.push(Asked {
                        section,
                        number,
                        written,
                    });
                }
            }
            Answer::Failed { reason } => {
                tracing::warn!(
                    section_id = section.id.as_str(),
                    "the model wrote no questions for this section: {reason}"
                );
                unanswered += *count as usize;
            }
        }
    }
    Ok(AskedQuestions { asked, unanswered })
}

/// The section as a model reads it: its Markdown, and the notes for writers when it has any.
pub(super) fn material_of(section: &Section) -> String {
    if section.notes.is_empty() {
        return section.markdown.clone();
    }
    format!(
        "{}\n\nNotes for the writer, not part of the section:\n{}",
        section.markdown, section.notes
    )
}

// SMELL: this spelling belongs on the task and reasoning types, beside the records that hold them.
// It sits here only because the other files of this folder already import this one.
/// The spelling the files use for a task, a kind of reasoning or any other word of a record, such
/// as `calculation`.
pub(super) fn json_name<T: Serialize>(value: T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}
