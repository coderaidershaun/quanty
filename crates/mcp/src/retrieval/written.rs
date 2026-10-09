//! What `answer` gives back: the claims of the answer and the items that they rest on.

use std::collections::BTreeMap;

use rag_core::UsageTally;
use rag_retrieval::Answer;
use schemars::JsonSchema;
use serde::Serialize;

use super::found::ItemView;
use crate::usage::Spent;

/// An answer written from the stored items, with the items it rests on.
#[derive(Serialize, JsonSchema)]
pub(crate) struct AnswerResult {
    /// False when no item was found, or the items found do not answer the question.
    answered: bool,
    /// A few words that name the answer.
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    /// The statements of the answer, in reading order.
    claims: Vec<ClaimView>,
    /// Every item that a claim rests on, once, by number.
    sources: Vec<SourceView>,
    /// Questions to ask next, each worded so that it can be asked on its own.
    follow_ups: Vec<String>,
    /// What the search and the answer used.
    #[serde(flatten)]
    spent: Spent,
}

#[derive(Serialize, JsonSchema)]
struct ClaimView {
    /// The name of the part of the answer that starts here, such as "Key assumptions".
    #[serde(skip_serializing_if = "Option::is_none")]
    heading: Option<String>,
    /// One or two sentences. A symbol in it is LaTeX between `\(` and `\)`.
    text: String,
    /// The numbers of the `sources` that this statement rests on.
    sources: Vec<usize>,
}

#[derive(Serialize, JsonSchema)]
struct SourceView {
    /// The number that a claim names.
    number: usize,
    #[serde(flatten)]
    item: ItemView,
}

impl AnswerResult {
    /// Nothing was found, so the model was not asked, and only the search was used.
    pub(super) fn unanswered(searched: &UsageTally) -> AnswerResult {
        AnswerResult {
            answered: false,
            title: None,
            claims: Vec::new(),
            sources: Vec::new(),
            follow_ups: Vec::new(),
            spent: searched.into(),
        }
    }
}

impl From<Answer> for AnswerResult {
    fn from(answer: Answer) -> AnswerResult {
        let mut sources: BTreeMap<usize, SourceView> = BTreeMap::new();
        let claims: Vec<ClaimView> = answer
            .claims
            .into_iter()
            .map(|claim| {
                let numbers = claim.sources.iter().map(|source| source.number).collect();
                for source in claim.sources {
                    sources.entry(source.number).or_insert_with(|| SourceView {
                        number: source.number,
                        item: (&source.payload).into(),
                    });
                }
                ClaimView {
                    heading: claim.heading,
                    text: claim.text,
                    sources: numbers,
                }
            })
            .collect();
        AnswerResult {
            answered: !claims.is_empty(),
            title: answer.title,
            claims,
            sources: sources.into_values().collect(),
            follow_ups: answer.follow_ups,
            spent: (&answer.usage).into(),
        }
    }
}
