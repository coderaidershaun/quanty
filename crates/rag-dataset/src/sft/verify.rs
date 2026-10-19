//! Decides whether an answer is kept: by the agreement of the tries and a judge, by a run of the
//! program that answers, or by a judge alone.

use rag_core::Llm;
use serde::Deserialize;

use crate::generate::asker::{Answer, AskError, Asker};
use crate::generate::role::Role;
use crate::sample::{Task, Verification, VerificationMethod};

use super::python::{self, ProgramError};
use super::questions::{Asked, material_of};
use super::solve::{Sample, Solved};

/// Judges one answer against the section its question was written from.
pub const JUDGE_ANSWER: Role = Role {
    id: "judge_answer",
    version: 1,
    system_prompt: include_str!("prompts/judge_answer.md"),
    schema: include_str!("prompts/judge_answer.schema.json"),
};

/// Two numbers agree when they differ by no more than this share of the larger one.
const RELATIVE_TOLERANCE: f64 = 1e-3;
const LOWEST_PASSING_SCORE: i64 = 4;

/// Why a question did not make an example.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Failure {
    /// No solver gave an answer.
    NoAnswer,
    /// Too few of the tries agree on the final value.
    Disagreement,
    /// The judge found the answer wrong, or gave no verdict.
    Judge,
    /// No program printed the value, or none could run.
    PythonExec,
}

impl Failure {
    /// The spelling the manifest and the rejected file use.
    pub(super) fn name(self) -> &'static str {
        match self {
            Failure::NoAnswer => "no_answer",
            Failure::Disagreement => "disagreement",
            Failure::Judge => "judge",
            Failure::PythonExec => "python_exec",
        }
    }
}

/// The result of every check of one question.
pub(super) struct Verified {
    method: VerificationMethod,
    samples_asked: u32,
    /// How many tries passed the check of the method: the ones that agree, the programs that
    /// printed the value, or 1 for a judge that passed.
    agreement: u32,
    judge_score: Option<u8>,
    /// The try the example is made from, or the best one when the question failed.
    pub(super) kept: Option<Sample>,
    pub(super) failure: Option<Failure>,
}

impl Verified {
    /// The only place that says whether a question passed, so that the verdict and the reason for
    /// failing cannot differ.
    pub(super) fn verification(&self) -> Verification {
        Verification {
            method: self.method,
            passed: self.failure.is_none(),
            n_teacher_samples: self.samples_asked,
            agreement: self.agreement,
            judge_score: self.judge_score,
        }
    }
}

/// What the checks that ask no model found for one question.
pub(super) enum Outcome {
    Settled(Verified),
    /// Passed so far, and waiting for the judge.
    ForJudge(Unjudged),
}

/// A question that passed the checks that ask no model, and the try that the judge is shown.
pub(super) struct Unjudged {
    method: VerificationMethod,
    samples_asked: u32,
    agreement: u32,
    kept: Sample,
}

impl Unjudged {
    /// A judge that gives no verdict fails the question, the same as one that finds fault.
    fn judged(self, verdict: Option<Answer<Judgement>>) -> Verified {
        let judgement = match verdict {
            Some(Answer::Given(judgement)) => Some(judgement),
            Some(Answer::Failed { reason }) => {
                tracing::warn!("the judge gave no verdict: {reason}");
                None
            }
            None => None,
        };
        let passed = judgement.as_ref().is_some_and(Judgement::passes);
        let agreement = if self.method == VerificationMethod::Judge {
            u32::from(passed)
        } else {
            self.agreement
        };
        Verified {
            method: self.method,
            samples_asked: self.samples_asked,
            agreement,
            judge_score: judgement.as_ref().map(Judgement::score),
            kept: Some(self.kept),
            failure: (!passed).then_some(Failure::Judge),
        }
    }
}

#[derive(Deserialize)]
struct Judgement {
    faithful: bool,
    unsupported_claims: Vec<String>,
    score: i64,
}

impl Judgement {
    fn passes(&self) -> bool {
        self.faithful && self.unsupported_claims.is_empty() && self.score >= LOWEST_PASSING_SCORE
    }

    fn score(&self) -> u8 {
        self.score.clamp(1, 5) as u8
    }
}

fn method_of(task: Task, run_code: bool) -> VerificationMethod {
    match task {
        Task::Calculation | Task::Derivation => VerificationMethod::AgreementAndJudge,
        Task::CodePot if run_code => VerificationMethod::PythonExec,
        Task::CodePot | Task::Explain | Task::Compare => VerificationMethod::Judge,
    }
}

/// The checks that ask no model: the agreement of the final values, or the run of the programs.
/// The outcomes come in the order of the questions.
///
/// # Errors
/// [`ProgramError`] when a program cannot be written to a temporary folder.
pub(super) async fn run_checks(
    asked: &[Asked<'_>],
    solved: Vec<Solved>,
    run_code: bool,
) -> Result<Vec<Outcome>, ProgramError> {
    let mut outcomes = Vec::with_capacity(solved.len());
    for (question, solved) in asked.iter().zip(solved) {
        let method = method_of(question.written.task, run_code);
        outcomes.push(check(method, solved).await?);
    }
    Ok(outcomes)
}

async fn check(method: VerificationMethod, solved: Solved) -> Result<Outcome, ProgramError> {
    let Solved {
        samples_asked,
        samples,
    } = solved;
    let Some(shortest_answered) = shortest(samples.iter()) else {
        return Ok(Outcome::Settled(Verified {
            method,
            samples_asked,
            agreement: 0,
            judge_score: None,
            kept: None,
            failure: Some(Failure::NoAnswer),
        }));
    };
    match method {
        VerificationMethod::AgreementAndJudge => {
            Ok(agreed(samples_asked, &samples, shortest_answered))
        }
        VerificationMethod::PythonExec => {
            executed(samples_asked, &samples, shortest_answered).await
        }
        VerificationMethod::Judge => Ok(Outcome::ForJudge(Unjudged {
            method,
            samples_asked,
            agreement: 0,
            kept: shortest_answered.clone(),
        })),
    }
}

/// A majority of the tries asked must agree on the final value. The example is made from the
/// shortest try of the biggest group, and from the shortest try when no try states a final value.
fn agreed(samples_asked: u32, samples: &[Sample], shortest_answered: &Sample) -> Outcome {
    let group = biggest_group(samples);
    let agreement = group.len() as u32;
    let kept = shortest(group.into_iter())
        .unwrap_or(shortest_answered)
        .clone();
    let method = VerificationMethod::AgreementAndJudge;
    if agreement * 2 > samples_asked {
        Outcome::ForJudge(Unjudged {
            method,
            samples_asked,
            agreement,
            kept,
        })
    } else {
        Outcome::Settled(Verified {
            method,
            samples_asked,
            agreement,
            judge_score: None,
            kept: Some(kept),
            failure: Some(Failure::Disagreement),
        })
    }
}

/// The biggest group of tries whose final values agree with the final value of one of them. A tie
/// goes to the group of the earlier try.
fn biggest_group(samples: &[Sample]) -> Vec<&Sample> {
    let mut biggest = Vec::new();
    for sample in samples {
        let Some(value) = sample.final_value.as_deref() else {
            continue;
        };
        let group: Vec<&Sample> = samples
            .iter()
            .filter(|other| {
                other
                    .final_value
                    .as_deref()
                    .is_some_and(|other_value| values_agree(value, other_value))
            })
            .collect();
        if group.len() > biggest.len() {
            biggest = group;
        }
    }
    biggest
}

/// Numbers agree when they are close. Anything else agrees when it is the same text once spaces and
/// dollar signs are gone and the capitals are lower case.
fn values_agree(first: &str, second: &str) -> bool {
    if let (Ok(first), Ok(second)) = (first.trim().parse::<f64>(), second.trim().parse::<f64>())
        && (first - second).abs() <= RELATIVE_TOLERANCE * first.abs().max(second.abs())
    {
        return true;
    }
    plain_text(first) == plain_text(second)
}

fn plain_text(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace() && *character != '$')
        .flat_map(char::to_lowercase)
        .collect()
}

/// Runs the program of each try. A try passes when its program prints the value that the try
/// states. The example is made from the shortest try that passed, and a failure keeps the shortest
/// try.
async fn executed(
    samples_asked: u32,
    samples: &[Sample],
    shortest_answered: &Sample,
) -> Result<Outcome, ProgramError> {
    let mut passed = Vec::new();
    for sample in samples {
        let Some(program) = python_block(&sample.answer) else {
            continue;
        };
        if python::passes(&program, sample.final_value.as_deref()).await? {
            passed.push(sample);
        }
    }
    let agreement = passed.len() as u32;
    let kept = shortest(passed.into_iter())
        .unwrap_or(shortest_answered)
        .clone();
    Ok(Outcome::Settled(Verified {
        method: VerificationMethod::PythonExec,
        samples_asked,
        agreement,
        judge_score: None,
        kept: Some(kept),
        failure: (agreement == 0).then_some(Failure::PythonExec),
    }))
}

/// The lines of the first fenced block of Python, up to the line of backticks that closes it, or
/// to the end of the answer when nothing closes it.
fn python_block(answer: &str) -> Option<String> {
    let mut lines = answer.lines();
    lines.find(|line| is_python_fence(line))?;
    let program: Vec<&str> = lines.take_while(|line| !is_closing_fence(line)).collect();
    Some(program.join("\n"))
}

fn is_python_fence(line: &str) -> bool {
    let line = line.trim();
    let language = line.trim_start_matches('`');
    line.len() - language.len() >= 3 && language.trim().eq_ignore_ascii_case("python")
}

fn is_closing_fence(line: &str) -> bool {
    let line = line.trim();
    line.len() >= 3 && line.chars().all(|character| character == '`')
}

/// The shortest of the tries. The earlier one wins a tie.
fn shortest<'a>(samples: impl Iterator<Item = &'a Sample>) -> Option<&'a Sample> {
    samples.min_by_key(|sample| sample.length())
}

/// Asks the judge about every question that waits for it, all of the step in one go, and settles
/// them. The verdicts come in the order of the questions.
///
/// # Errors
/// The errors of [`Asker::ask_each`].
pub(super) async fn judge<L: Llm>(
    asker: &Asker<'_, L>,
    asked: &[Asked<'_>],
    outcomes: Vec<Outcome>,
) -> Result<Vec<Verified>, AskError> {
    let inputs: Vec<String> = asked
        .iter()
        .zip(&outcomes)
        .filter_map(|(question, outcome)| match outcome {
            Outcome::ForJudge(unjudged) => Some(judge_input(question, &unjudged.kept)),
            Outcome::Settled(_) => None,
        })
        .collect();
    let mut verdicts = asker
        .ask_each::<Judgement>(&JUDGE_ANSWER, &inputs)
        .await?
        .into_iter();
    Ok(outcomes
        .into_iter()
        .map(|outcome| match outcome {
            Outcome::Settled(verified) => verified,
            Outcome::ForJudge(unjudged) => unjudged.judged(verdicts.next()),
        })
        .collect())
}

fn judge_input(question: &Asked<'_>, kept: &Sample) -> String {
    let working = kept
        .thinking
        .as_deref()
        .map(|thinking| format!("Working:\n{thinking}\n\n"))
        .unwrap_or_default();
    format!(
        "Section:\n{}\n\nQuestion:\n{}\n\n{working}Answer:\n{}",
        material_of(question.section),
        question.written.question,
        kept.answer
    )
}
