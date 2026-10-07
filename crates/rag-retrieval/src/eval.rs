//! Scores the golden questions: how many of them bring back the page that answers them.

use std::fmt;
use std::path::{Path, PathBuf};

use graph::GraphStore;
use rag_core::Embedder;

use crate::search::{RESULTS_PER_QUERY, Retriever, SearchError};

/// How many results of a question are looked at: the first ones, as a person sees them. When a
/// search ranks fewer items than this, the items that its results cite come next, so one of those
/// can be among them and make a question count as found.
const TOP_RESULTS: usize = 5;

// A search must give at least as many results as the score looks at. With fewer, the score would
// quietly be for a smaller number of results than the one it prints.
const _: () = assert!(RESULTS_PER_QUERY >= TOP_RESULTS);

/// A page of a document that a question's answer is on.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoldenPlace {
    /// The title of the document, as it is stored with every item.
    pub document: String,
    /// The position of the page in the chapter, from 1. Not the printed page number.
    pub page: u32,
}

/// One question and where its answer is.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoldenQuestion {
    pub text: String,
    /// The title of the document that must come back, as it is stored with every item.
    pub document: String,
    /// The position of the page in the chapter, from 1. Not the printed page number.
    pub page: u32,
    /// More places that must come back too, for a question whose answer is spread over documents.
    #[serde(default)]
    pub also: Vec<GoldenPlace>,
}

impl GoldenQuestion {
    /// Every place that must come back: the page of `document`, then each of `also`.
    fn places(&self) -> impl Iterator<Item = (&str, u32)> {
        std::iter::once((self.document.as_str(), self.page)).chain(
            self.also
                .iter()
                .map(|place| (place.document.as_str(), place.page)),
        )
    }
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
/// `document` and a `page`, and an `also` list of more places when the answer is spread over
/// documents.
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

/// One golden question and whether its expected pages came back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionOutcome {
    pub question: GoldenQuestion,
    /// The place, from 1, by which every expected page has come back: the largest of the places
    /// where each of them first shows up. `None` when one of them is not among the top results.
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
                None => {
                    let expected = question
                        .places()
                        .map(|(document, page)| format!("{document}, page {page} of the chapter"))
                        .collect::<Vec<_>>()
                        .join("; and ");
                    writeln!(formatter, "missed: {one_line} (expected {expected})")?
                }
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
/// notes where the expected pages show up in the top results.
///
/// # Errors
/// The first failed search ends the run. There is no partial score.
pub async fn evaluate<E: Embedder, G: GraphStore>(
    golden: &[GoldenQuestion],
    retriever: &Retriever<E, G>,
) -> Result<EvalReport, SearchError> {
    let mut outcomes = Vec::with_capacity(golden.len());
    for question in golden {
        let results = retriever.search(&question.text, None).await?;
        let top = &results.hits[..results.hits.len().min(TOP_RESULTS)];
        let found_at = question
            .places()
            .map(|(document, page)| {
                top.iter()
                    .position(|hit| {
                        let payload = &hit.item.payload;
                        payload.doc_title == document && payload.page == page
                    })
                    .map(|index| index + 1)
            })
            .collect::<Option<Vec<usize>>>()
            .and_then(|places| places.into_iter().max());
        outcomes.push(QuestionOutcome {
            question: question.clone(),
            found_at,
        });
    }
    Ok(EvalReport { outcomes })
}
