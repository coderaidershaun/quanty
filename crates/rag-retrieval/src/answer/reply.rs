//! Reads and checks what the model replied, and prints the answer. The program, not the model,
//! writes the document, the printed page and the LaTeX of each source.

use std::fmt;

use rag_core::{ItemKind, ItemPayload};
use serde::Deserialize;
use serde_json::Value;

use super::AnswerError;
use crate::search::{SearchResults, page_text};

/// What the schema of the reply asks for.
#[derive(Deserialize)]
struct Reply {
    claims: Vec<ReplyClaim>,
}

#[derive(Deserialize)]
struct ReplyClaim {
    text: String,
    sources: Vec<i64>,
}

/// One statement of an answer, and the items that it rests on.
#[derive(Debug, Clone, PartialEq)]
pub struct Claim {
    pub text: String,
    pub sources: Vec<ItemPayload>,
}

/// An answer written from the items that a search found. It prints each claim with its sources:
/// the document, the printed page and the kind of each item, the unchanged LaTeX of a formula and
/// the path of the picture of a figure.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    /// In reading order. Empty when the items do not answer the question.
    pub claims: Vec<Claim>,
}

/// The claims of the reply, each with the items that its source numbers name. A source that a
/// claim names twice is kept once.
///
/// # Errors
/// - [`AnswerError::Unreadable`] when the reply is not a list of claims
/// - [`AnswerError::NoSource`] when a claim names no source
/// - [`AnswerError::UnknownSource`] when a claim names a number that is not the number of an item
pub(super) fn read(reply: Value, results: &SearchResults) -> Result<Answer, AnswerError> {
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
            .map(|place| results.hits[place - 1].item.payload.clone())
            .collect();
        claims.push(Claim {
            text: claim.text,
            sources,
        });
    }
    Ok(Answer { claims })
}

impl fmt::Display for Answer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.claims.is_empty() {
            return formatter.write_str("the stored items do not answer the question");
        }
        for (index, claim) in self.claims.iter().enumerate() {
            if index > 0 {
                formatter.write_str("\n\n")?;
            }
            formatter.write_str(&claim.text)?;
            for source in &claim.sources {
                write_source(formatter, source)?;
            }
        }
        Ok(())
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
