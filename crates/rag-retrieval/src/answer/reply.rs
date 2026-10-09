//! Reads and checks what the model replied, and prints the answer. The program, not the model,
//! writes the document, the printed page and the LaTeX of each source.

use std::fmt;

use rag_core::{ItemKind, ItemPayload, UsageTally};
use serde::Deserialize;
use serde_json::Value;

use super::AnswerError;
use crate::search::{SearchResults, page_text, write_usage};

/// How many follow-up questions an answer keeps. The prompt and the comment on
/// `Answer::follow_ups` say this number too, so change the three together.
const MAX_FOLLOW_UPS: usize = 4;

/// What the schema of the reply asks for. Only the claims must be there: a reply without the rest
/// reads as an answer with no title, no heading and no follow-up question.
#[derive(Deserialize)]
struct Reply {
    claims: Vec<ReplyClaim>,
    #[serde(default)]
    title: String,
    #[serde(default)]
    follow_ups: Vec<String>,
}

#[derive(Deserialize)]
struct ReplyClaim {
    #[serde(default)]
    heading: String,
    text: String,
    sources: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    /// The place of the item in the results that the answer was written from, counted from 1.
    pub number: usize,
    pub payload: ItemPayload,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Claim {
    /// The name of the part of the answer that starts at this claim, such as "Key assumptions".
    /// Most claims have none.
    pub heading: Option<String>,
    /// One or two sentences. A symbol in it is LaTeX between `\(` and `\)`.
    pub text: String,
    pub sources: Vec<Source>,
}

/// The title, the headings and the follow-up questions are not printed.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    /// A few words that name the answer. `None` when the model gave none, and always when there
    /// are no claims.
    pub title: Option<String>,
    /// In reading order. Empty when the items do not answer the question.
    pub claims: Vec<Claim>,
    /// At most four questions to ask next. The model is told to write each one so that it can be
    /// asked on its own.
    pub follow_ups: Vec<String>,
    /// What the question used: the search and the answer.
    pub usage: UsageTally,
}

/// A source that a claim names twice is kept once. A title, a heading or a follow-up question that
/// is blank is left out, and never an error. `usage` is what the question used.
///
/// # Errors
/// - [`AnswerError::Unreadable`] when the reply is not a list of claims
/// - [`AnswerError::NoSource`] when a claim names no source
/// - [`AnswerError::UnknownSource`] when a claim names a number that is not the number of an item
pub(super) fn read(
    reply: Value,
    results: &SearchResults,
    usage: UsageTally,
) -> Result<Answer, AnswerError> {
    let reply: Reply = serde_json::from_value(reply).map_err(AnswerError::Unreadable)?;
    let item_count = results.hits.len();
    let mut claims = Vec::with_capacity(reply.claims.len());
    for (index, claim) in reply.claims.into_iter().enumerate() {
        let claim_number = index + 1;
        if claim.sources.is_empty() {
            return Err(AnswerError::NoSource {
                claim: claim_number,
            });
        }
        let mut places: Vec<usize> = Vec::new();
        for source in claim.sources {
            let place = usize::try_from(source)
                .ok()
                .filter(|place| (1..=item_count).contains(place))
                .ok_or(AnswerError::UnknownSource {
                    claim: claim_number,
                    number: source,
                    items: item_count,
                })?;
            if !places.contains(&place) {
                places.push(place);
            }
        }
        let sources = places
            .into_iter()
            .map(|number| Source {
                number,
                payload: results.hits[number - 1].item.payload.clone(),
            })
            .collect();
        claims.push(Claim {
            heading: one_line(&claim.heading),
            text: with_backslashes_restored(&claim.text),
            sources,
        });
    }
    let title = if claims.is_empty() {
        None
    } else {
        one_line(&reply.title)
    };
    Ok(Answer {
        title,
        claims,
        follow_ups: kept_follow_ups(&reply.follow_ups),
        usage,
    })
}

fn kept_follow_ups(asked: &[String]) -> Vec<String> {
    let mut kept: Vec<String> = Vec::new();
    for question in asked.iter().filter_map(|text| one_line(text)) {
        if kept.len() < MAX_FOLLOW_UPS && !kept.contains(&question) {
            kept.push(question);
        }
    }
    kept
}

/// The text on one line, or `None` when it holds no word.
fn one_line(text: &str) -> Option<String> {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!line.is_empty()).then_some(line)
}

/// Puts back the backslash of a LaTeX command that the model wrote with one backslash. JSON reads
/// `\theta` written that way as a tab and `heta`, and does the same with a command that starts
/// with `b`, `f` or `r`. A sentence holds none of those four control characters for any other
/// reason.
///
/// A command that starts with `n`, such as `\nu`, arrives as a line break and `u`. A line break
/// can also be one that the model meant, so it is put back only between `\(` and `\)` and before
/// a letter: the prompt allows no line break between those marks.
fn with_backslashes_restored(text: &str) -> String {
    let mut restored = String::with_capacity(text.len());
    let mut is_in_math = false;
    let mut follows_backslash = false;
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\t' => restored.push_str("\\t"),
            '\u{8}' => restored.push_str("\\b"),
            '\u{c}' => restored.push_str("\\f"),
            '\r' => restored.push_str("\\r"),
            '\n' if is_in_math && characters.peek().is_some_and(char::is_ascii_alphabetic) => {
                restored.push_str("\\n");
            }
            other => restored.push(other),
        }
        // A backslash and the character after it are read as one, so the `(` of `\\(` opens
        // nothing.
        if follows_backslash {
            match character {
                '(' => is_in_math = true,
                ')' => is_in_math = false,
                _ => {}
            }
        }
        follows_backslash = character == '\\' && !follows_backslash;
    }
    restored
}

impl fmt::Display for Answer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.claims.is_empty() {
            formatter.write_str("the stored items do not answer the question")?;
        }
        for (index, claim) in self.claims.iter().enumerate() {
            if index > 0 {
                formatter.write_str("\n\n")?;
            }
            formatter.write_str(&claim.text)?;
            for source in &claim.sources {
                write_source(formatter, &source.payload)?;
            }
        }
        write_usage(formatter, &self.usage)
    }
}

fn write_source(formatter: &mut fmt::Formatter<'_>, source: &ItemPayload) -> fmt::Result {
    write!(
        formatter,
        "\n  source: {}, page {} ({}",
        source.doc_title,
        page_text(source),
        source.kind.as_str()
    )?;
    if let Some(label) = &source.label {
        write!(formatter, " {label}")?;
    }
    formatter.write_str(")")?;
    match source.kind {
        ItemKind::Formula => write!(formatter, "\n{}", source.text),
        ItemKind::Figure => match &source.image_path {
            Some(picture) => write!(formatter, "\n  picture: {}", picture.display()),
            None => Ok(()),
        },
        ItemKind::Chunk | ItemKind::Table => Ok(()),
    }
}
