//! Writes an answer to a question from the items that a search found. The model names the items
//! that support each statement by number, and the program prints their documents, pages and
//! formulas itself, so the model never retypes a title, a page or a formula.

mod message;
mod reply;

use rag_core::{Llm, LlmError, Question};

use crate::search::SearchResults;

pub use reply::{Answer, Claim};

const SYSTEM_PROMPT: &str = include_str!("prompts/answer.md");
const SCHEMA: &str = include_str!("prompts/answer.schema.json");

/// The model that writes the answers.
// Pinned: the prompt was written against this model, so a change of the default model must not
// change the answers.
pub const ANSWER_MODEL: &str = "claude-sonnet-5-5";

#[derive(thiserror::Error, Debug)]
pub enum AnswerError {
    #[error("could not get an answer from the model")]
    Llm(#[from] LlmError),

    #[error("the model's answer is not a list of claims, each with a text and its source numbers")]
    Unreadable(#[source] serde_json::Error),

    #[error("claim {claim} of the answer names no source; each claim must name at least one item")]
    NoSource { claim: usize },

    #[error(
        "claim {claim} of the answer names item {number} as a source, but {items} items were given, numbered from 1"
    )]
    UnknownSource {
        claim: usize,
        number: i64,
        items: usize,
    },
}

/// Asks the model for an answer to the question from the items of the results. It makes one call,
/// and a reply that breaks a rule is an error and is not asked for again. An answer with no
/// claims means the items do not answer the question.
///
/// # Errors
/// - [`AnswerError::Llm`] when the model cannot be asked
/// - [`AnswerError::Unreadable`], [`AnswerError::NoSource`] and [`AnswerError::UnknownSource`]
///   when the reply breaks a rule
pub async fn answer(
    llm: &impl Llm,
    question: &str,
    results: &SearchResults,
) -> Result<Answer, AnswerError> {
    let input = message::input_for(question, results);
    let reply = llm
        .ask(Question {
            system_prompt: SYSTEM_PROMPT,
            schema: SCHEMA,
            input: &input,
        })
        .await?;
    reply::read(reply, results)
}
