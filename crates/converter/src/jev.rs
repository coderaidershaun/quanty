//! Finds out whether a page of extracted PDF text holds symbolic math, and where it sits, so the
//! math can later be written as LaTeX. The Jev API makes the judgement.

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

const API_KEY_VARIABLE: &str = "CONVERTER_JEV_API_KEY";
const ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";
// Pinned because the questions were tuned against this version; a newer one may answer differently.
const MODEL: &str = "jev-1.13.0";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
// This is low on purpose: a damaged formula scores well below a confident yes, ordinary lines
// score lower still, and missing math costs more than a false alarm.
const FORMULA_YES_THRESHOLD: f64 = 0.3;
const WORDS_YES_THRESHOLD: f64 = 0.5;

const FORMULA_QUESTION: &str = "Does `line` contain a `symbolic_formula`, as described in `guide`?";
const WORDS_QUESTION: &str = "Does `line` contain words of an ordinary sentence, not only a heading, label, caption, number or formula?";

/// Client for the Jev API. Build one and reuse it for every page, so connections are shared.
#[derive(Clone)]
pub struct Jev {
    http: reqwest::Client,
    api_key: Box<str>,
}

/// Where the math on a page sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MathPlacement {
    /// Math written inside sentences.
    Inline,
    /// Math on lines of its own.
    Block,
    /// Both of the above.
    Both,
}

/// Why a Jev request failed.
#[derive(thiserror::Error, Debug)]
pub enum JevError {
    /// The environment variable that holds the API key is not set, or is not valid Unicode.
    #[error("environment variable {API_KEY_VARIABLE} is not set, or is not valid Unicode")]
    MissingApiKey,

    /// The client could not be built, the server could not be reached, or the request timed out.
    #[error("jev request failed")]
    Http(#[from] reqwest::Error),

    /// The server answered with a non-success status.
    #[error("jev rejected the request with status {status}: {body}")]
    Rejected {
        /// The HTTP status code, such as 401 for a bad key or 429 when rate limited.
        status: u16,
        /// The response body, where the server says what it objected to.
        body: String,
    },

    /// The server answered with success but the body was not the expected shape.
    #[error("jev response could not be read: {body}")]
    Decode {
        /// The response body as it arrived.
        body: String,
        /// What was wrong with the body.
        #[source]
        source: serde_json::Error,
    },
}

// Written by hand so the API key never shows up in logs or debug output. Do not derive it.
impl fmt::Debug for Jev {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Jev").finish_non_exhaustive()
    }
}

impl Jev {
    /// Builds a client that sends `api_key` as a bearer token.
    ///
    /// # Errors
    /// - [`JevError::Http`] if the HTTP client cannot be built.
    pub fn new(api_key: impl Into<String>) -> Result<Self, JevError> {
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()?;
        Ok(Self {
            http,
            api_key: api_key.into().into_boxed_str(),
        })
    }

    /// Builds a client from `CONVERTER_JEV_API_KEY` in the process environment.
    ///
    /// # Errors
    /// - [`JevError::MissingApiKey`] if the variable is not set or is not valid Unicode.
    /// - [`JevError::Http`] if the HTTP client cannot be built.
    pub fn from_env() -> Result<Self, JevError> {
        let api_key = std::env::var(API_KEY_VARIABLE).map_err(|_| JevError::MissingApiKey)?;
        Self::new(api_key)
    }

    /// Reports whether `page` holds symbolic math and where it sits.
    ///
    /// `page` is the text extracted from one PDF page, with its line breaks kept. Each non-blank
    /// line is judged on its own, all in one request. Returns `None` when no line holds a
    /// formula. Plain numbers, arithmetic on numbers, formulas made of ordinary words and a
    /// single letter standing alone in a sentence do not count; spelled-out Greek letter names
    /// joined by an operator do. A page with no visible text returns `None` without calling
    /// the API.
    ///
    /// # Errors
    /// - [`JevError::Http`] if the server cannot be reached or the request times out.
    /// - [`JevError::Rejected`] if the server answers with a non-success status, such as 401 for
    ///   a bad key or 429 when rate limited. Retrying is up to the caller.
    /// - [`JevError::Decode`] if a success response is missing an answer or is not valid JSON.
    pub async fn contains_math(&self, page: &str) -> Result<Option<MathPlacement>, JevError> {
        let lines: Vec<&str> = page
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect();
        // The server refuses a request with no questions, and a page with no text has no math.
        if lines.is_empty() {
            return Ok(None);
        }
        let response = self
            .http
            .post(ENDPOINT)
            .bearer_auth(&*self.api_key)
            .json(&request_body(&lines))
            .send()
            .await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(JevError::Rejected {
                status: status.as_u16(),
                body,
            });
        }
        placement_from_body(&body, lines.len())
    }
}

// SMELL: the whole page goes out as one request with two questions per line, and nothing here
// splits a long page. A page of several hundred lines can exceed the model's request size limit.
fn request_body(lines: &[&str]) -> Value {
    let questions: serde_json::Map<String, Value> = lines
        .iter()
        .enumerate()
        .flat_map(|(line_index, line)| {
            let formula_question = yes_no_question(line, FORMULA_QUESTION);
            let words_question = yes_no_question(line, WORDS_QUESTION);
            [
                (formula_id(line_index), formula_question),
                (words_id(line_index), words_question),
            ]
        })
        .collect();
    let mut body = json!({
        "state": { "guide": guide() },
        "model": MODEL,
        "questions": questions,
    });
    // Do not remove: the questions were tuned with the keys in sorted order, and `serde_json`
    // now keeps insertion order instead.
    body.sort_all_objects();
    body
}

fn formula_id(line_index: usize) -> String {
    format!("formula_{line_index}")
}

fn words_id(line_index: usize) -> String {
    format!("words_{line_index}")
}

// Each question carries its own copy of the line. Sending the lines once in `state` and pointing
// each question at one of them made the answers far less reliable.
fn yes_no_question(line: &str, question: &str) -> Value {
    json!({
        "type": "noul",
        "instructions": { "line": line, "question": question },
    })
}

// The examples must not quote text from the sample pages, or the live test would pass by string match.
fn guide() -> Value {
    json!({
        "symbolic_formula": "An expression whose terms are variables: single letters or short clusters of letters and digits, Greek symbols, or the spelled-out names of Greek letters (alpha, sigma), joined by operators, equals signs, brackets, bars, fractions, sums, subscripts or superscripts. A single piece of such an expression counts too.",
        "damaged_form": "Each line was extracted from a PDF, so a formula often arrives damaged: subscripts and superscripts drop onto the line, symbols turn into wrong characters, and terms run together with punctuation and few spaces.",
        "examples": ["f(x) = ax2 + bx + c", "Pr(Xt+1 < k | Xt)", "s2 = 1/n E(xi — m)2", "dS = mS dt + oS dW", "2k/(k + 1)", "alpha/sigma"],
        "is_not": "Arithmetic on plain numbers, money, percentages, dates, rows of numbers, chart tick marks, a number that labels an equation, section, figure, table or page, a single letter used as a name, and an expression whose terms are ordinary English words about things (not Greek letter names).",
        "is_not_examples": ["6% x $50,000 = $3,000", "profit = revenue — costs", "price/earnings", "(2.14)", "Figure 3-2", "+10   +20   +30", "the t-test", "in n steps"],
    })
}

#[derive(Deserialize)]
struct Response {
    answers: HashMap<String, Answer>,
}

#[derive(Deserialize)]
struct Answer {
    noul: f64,
}

impl Response {
    fn is_yes(&self, id: &str, yes_threshold: f64) -> Result<bool, serde_json::Error> {
        let answer = self
            .answers
            .get(id)
            .ok_or_else(|| serde::de::Error::custom(format!("no answer for {id}")))?;
        Ok(answer.noul > yes_threshold)
    }
}

// Every line that was asked about must come back with both answers. A missing answer is an
// error, because skipping the line would quietly report a page with math as having none.
fn placement_from_body(body: &str, line_count: usize) -> Result<Option<MathPlacement>, JevError> {
    let decode_error = |source| JevError::Decode {
        body: body.to_owned(),
        source,
    };
    let response: Response = serde_json::from_str(body).map_err(decode_error)?;

    let mut has_inline = false;
    let mut has_block = false;
    for line_index in 0..line_count {
        let has_formula = response
            .is_yes(&formula_id(line_index), FORMULA_YES_THRESHOLD)
            .map_err(decode_error)?;
        let has_words = response
            .is_yes(&words_id(line_index), WORDS_YES_THRESHOLD)
            .map_err(decode_error)?;
        match (has_formula, has_words) {
            (true, true) => has_inline = true,
            (true, false) => has_block = true,
            (false, _) => {}
        }
    }

    Ok(match (has_inline, has_block) {
        (false, false) => None,
        (true, false) => Some(MathPlacement::Inline),
        (false, true) => Some(MathPlacement::Block),
        (true, true) => Some(MathPlacement::Both),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(answers: &[(&str, f64)]) -> String {
        let answers: serde_json::Map<String, Value> = answers
            .iter()
            .map(|(id, noul)| (id.to_string(), json!({"type": "noul", "noul": noul})))
            .collect();
        json!({
            "model": "jev-1.13.0",
            "answers": answers,
            "usage": {"input_tokens": 10, "output_tokens": 2},
        })
        .to_string()
    }

    #[test]
    fn placement_is_read_from_the_response_body() {
        let none = body(&[
            ("formula_0", 0.1),
            ("words_0", 0.9),
            ("formula_1", 0.2),
            ("words_1", 0.1),
        ]);
        let inline = body(&[
            ("formula_0", 0.9),
            ("words_0", 0.8),
            ("formula_1", 0.1),
            ("words_1", 0.1),
        ]);
        let block = body(&[
            ("formula_0", 0.1),
            ("words_0", 0.9),
            ("formula_1", 0.8),
            ("words_1", 0.2),
        ]);
        let both = body(&[
            ("formula_0", 0.9),
            ("words_0", 0.8),
            ("formula_1", 0.8),
            ("words_1", 0.2),
        ]);

        assert_eq!(placement_from_body(&none, 2).unwrap(), None);
        assert_eq!(
            placement_from_body(&inline, 2).unwrap(),
            Some(MathPlacement::Inline)
        );
        assert_eq!(
            placement_from_body(&block, 2).unwrap(),
            Some(MathPlacement::Block)
        );
        assert_eq!(
            placement_from_body(&both, 2).unwrap(),
            Some(MathPlacement::Both)
        );
    }
}
