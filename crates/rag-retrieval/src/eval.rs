//! Scores the golden questions: how many of them bring back the page that answers them.

use std::fmt;
use std::path::{Path, PathBuf};

use rag_core::Embedder;

use crate::search::{Retriever, SearchError};

/// How many results of a question are looked at.
const TOP_RESULTS: usize = 5;

/// One question and where its answer is.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoldenQuestion {
    pub text: String,
    /// The title of the document that must come back, as it is stored with every item.
    pub document: String,
    /// The position of the page in the chapter, from 1. Not the printed page number.
    pub page: u32,
}

#[derive(thiserror::Error, Debug)]
pub enum GoldenError {
    #[error("could not read the golden questions in {}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{} is not a list of golden questions", path.display())]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
}

/// The shape of the golden file: a list of tables that are all called `question`.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct GoldenFile {
    question: Vec<GoldenQuestion>,
}

/// Reads the golden questions from a file of `[[question]]` tables, each with a `text`, a
/// `document` and a `page`.
///
/// # Errors
/// - [`GoldenError::Read`] when the file cannot be read
/// - [`GoldenError::Parse`] when it holds no `[[question]]`, a table or key of another name, or a
///   question with a missing, unknown or wrongly typed field
pub fn read_golden_questions(path: &Path) -> Result<Vec<GoldenQuestion>, GoldenError> {
    let text = std::fs::read_to_string(path).map_err(|source| GoldenError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let file: GoldenFile = toml::from_str(&text).map_err(|source| GoldenError::Parse {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(file.question)
}

/// One golden question and whether its expected page came back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionOutcome {
    pub question: GoldenQuestion,
    /// The place, from 1, of the first result that is from the expected page. `None` when no
    /// result in the top results is from it.
    pub found_at: Option<usize>,
}

/// The outcome of each question, in the order asked. It prints one line for each question and a
/// last line with the score. A question that holds line breaks is printed with its white space
/// joined into single spaces, so that it still takes one line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalReport {
    pub outcomes: Vec<QuestionOutcome>,
}

impl EvalReport {
    pub fn found(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|outcome| outcome.found_at.is_some())
            .count()
    }

    pub fn asked(&self) -> usize {
        self.outcomes.len()
    }
}

impl fmt::Display for EvalReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for outcome in &self.outcomes {
            let question = &outcome.question;
            let one_line = question
                .text
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            match outcome.found_at {
                Some(place) => writeln!(formatter, "found at {place}: {one_line}")?,
                None => writeln!(
                    formatter,
                    "missed: {one_line} (expected {}, page {} of the chapter)",
                    question.document, question.page
                )?,
            }
        }
        write!(
            formatter,
            "found in the top {TOP_RESULTS}: {} of {}",
            self.found(),
            self.asked()
        )
    }
}

/// Asks every question through the same search that `rag-query` uses, one after the other, and
/// notes where the expected page first shows up in the top results.
///
/// # Errors
/// The first failed search ends the run. There is no partial score.
pub async fn evaluate<E: Embedder>(
    golden: &[GoldenQuestion],
    retriever: &Retriever<E>,
) -> Result<EvalReport, SearchError> {
    let mut outcomes = Vec::with_capacity(golden.len());
    for question in golden {
        let results = retriever.search(&question.text, None, TOP_RESULTS).await?;
        let found_at = results
            .hits
            .iter()
            .position(|hit| {
                hit.payload.doc_title == question.document && hit.payload.page == question.page
            })
            .map(|index| index + 1);
        outcomes.push(QuestionOutcome {
            question: question.clone(),
            found_at,
        });
    }
    Ok(EvalReport { outcomes })
}
