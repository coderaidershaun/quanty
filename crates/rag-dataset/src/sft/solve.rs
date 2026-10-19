//! The solvers: several tries with the working shown for a question of maths or code, and one
//! straight answer for an explanation.

use rag_core::Llm;
use serde::Deserialize;

use crate::generate::asker::{Answer, AskError, Asker};
use crate::generate::role::Role;
use crate::sample::{Reasoning, Task};

use super::questions::{Asked, json_name, material_of};

/// Solves one question with the working shown, and says the final value on its own.
pub const SOLVE_THINK: Role = Role {
    id: "solve_think",
    version: 1,
    system_prompt: include_str!("prompts/solve_think.md"),
    schema: include_str!("prompts/solve_think.schema.json"),
};

/// Answers one question straight out.
pub const SOLVE_DIRECT: Role = Role {
    id: "solve_direct",
    version: 1,
    system_prompt: include_str!("prompts/solve_direct.md"),
    schema: include_str!("prompts/solve_direct.schema.json"),
};

#[derive(Deserialize)]
struct ThinkReply {
    thinking: String,
    answer: String,
    final_value: Option<String>,
}

#[derive(Deserialize)]
struct DirectReply {
    answer: String,
}

/// One try at a question.
#[derive(Debug, Clone)]
pub(super) struct Sample {
    /// Set only for a try that showed its working.
    pub(super) thinking: Option<String>,
    pub(super) answer: String,
    pub(super) final_value: Option<String>,
}

impl Sample {
    /// How long the try is in characters, working and answer together.
    pub(super) fn length(&self) -> usize {
        let thinking = self
            .thinking
            .as_deref()
            .map_or(0, |text| text.chars().count());
        thinking + self.answer.chars().count()
    }
}

impl From<ThinkReply> for Sample {
    fn from(reply: ThinkReply) -> Sample {
        let final_value = reply
            .final_value
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        Sample {
            thinking: Some(reply.thinking),
            answer: reply.answer,
            final_value,
        }
    }
}

impl From<DirectReply> for Sample {
    fn from(reply: DirectReply) -> Sample {
        Sample {
            thinking: None,
            answer: reply.answer,
            final_value: None,
        }
    }
}

/// What the solvers gave for one question.
pub(super) struct Solved {
    /// How many tries were asked for, whether or not they answered.
    pub(super) samples_asked: u32,
    /// The tries that answered, in the order they were asked.
    pub(super) samples: Vec<Sample>,
}

pub(super) fn reasoning_of(task: Task) -> Reasoning {
    match task {
        Task::Derivation | Task::Calculation | Task::CodePot => Reasoning::Think,
        Task::Explain | Task::Compare => Reasoning::Direct,
    }
}

/// Asks the solvers for every question: first all the tries of the questions to think about, then
/// the straight answers. `samples` is how many tries a question to think about gets.
///
/// # Errors
/// The errors of [`Asker::ask_each`].
pub(super) async fn solve<L: Llm>(
    asker: &Asker<'_, L>,
    asked: &[Asked<'_>],
    samples: u32,
) -> Result<Vec<Solved>, AskError> {
    let mut think_inputs = Vec::new();
    let mut direct_inputs = Vec::new();
    for question in asked {
        match reasoning_of(question.written.task) {
            Reasoning::Think => {
                think_inputs.extend((1..=samples).map(|sample| input_for(question, Some(sample))));
            }
            Reasoning::Direct => direct_inputs.push(input_for(question, None)),
        }
    }
    let think_replies = asker
        .ask_each::<ThinkReply>(&SOLVE_THINK, &think_inputs)
        .await?;
    let direct_replies = asker
        .ask_each::<DirectReply>(&SOLVE_DIRECT, &direct_inputs)
        .await?;
    let mut think_samples = answered(think_replies);
    let mut direct_samples = answered(direct_replies);

    Ok(asked
        .iter()
        .map(|question| match reasoning_of(question.written.task) {
            Reasoning::Think => Solved {
                samples_asked: samples,
                samples: think_samples
                    .by_ref()
                    .take(samples as usize)
                    .flatten()
                    .collect(),
            },
            Reasoning::Direct => Solved {
                samples_asked: 1,
                samples: direct_samples.by_ref().take(1).flatten().collect(),
            },
        })
        .collect())
}

/// A try at a question to think about ends with its own number, so that the kept answers tell the
/// tries apart. The number carries no total, so a later build that asks for more tries finds the
/// first ones already kept.
fn input_for(question: &Asked<'_>, sample: Option<u32>) -> String {
    let sample_line = sample
        .map(|number| format!("\n\nSample {number}"))
        .unwrap_or_default();
    format!(
        "Task: {}\n\nQuestion:\n{}\n\nSection the question was written from:\n{}{sample_line}",
        json_name(question.written.task),
        question.written.question,
        material_of(question.section)
    )
}

/// The tries that answered, with a `None` for each that did not.
fn answered<T: Into<Sample>>(answers: Vec<Answer<T>>) -> impl Iterator<Item = Option<Sample>> {
    answers.into_iter().map(|answer| match answer {
        Answer::Given(reply) => Some(reply.into()),
        Answer::Failed { reason } => {
            tracing::warn!("a solver gave no answer: {reason}");
            None
        }
    })
}
