//! Every batch sees the original chapter and exact source inventory.

use std::collections::BTreeSet;

use ocr::content::{ArtifactTarget, ArtifactText, ArtifactTextError};
use serde::Deserialize;

pub(super) const PROMPT: &str = include_str!("prompts/metadata.md");
pub(super) const SCHEMA: &str = include_str!("prompts/metadata.schema.json");
/// Conservative local UTF-8 budget, not a provider token count.
pub(super) const MAX_BYTES: usize = 100_000;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    artifacts: Vec<ArtifactText>,
}

pub(super) fn validate(
    value: serde_json::Value,
    targets: &[ArtifactTarget],
) -> Result<Vec<ArtifactText>, ReplyError> {
    let reply: Reply = serde_json::from_value(value).map_err(ReplyError::Shape)?;
    let expected: BTreeSet<_> = targets.iter().map(|target| target.id.as_str()).collect();
    let mut seen = BTreeSet::new();
    for text in &reply.artifacts {
        if !expected.contains(text.id.as_str()) {
            return Err(ReplyError::Unknown {
                id: text.id.clone(),
            });
        }
        if !seen.insert(text.id.as_str()) {
            return Err(ReplyError::Duplicate {
                id: text.id.clone(),
            });
        }
        if let Err(source) = text.validate() {
            return Err(ReplyError::Text {
                id: text.id.clone(),
                source,
            });
        }
    }
    let missing: Vec<_> = expected
        .difference(&seen)
        .map(|id| (*id).to_owned())
        .collect();
    if !missing.is_empty() {
        return Err(ReplyError::Missing { ids: missing });
    }
    Ok(reply.artifacts)
}

#[derive(Debug, thiserror::Error)]
pub(super) enum ReplyError {
    #[error("invalid response shape: {0}")]
    Shape(#[source] serde_json::Error),
    #[error("unknown id {id:?}")]
    Unknown { id: String },
    #[error("duplicate id {id:?}")]
    Duplicate { id: String },
    #[error("invalid text for {id:?}: {source}")]
    Text {
        id: String,
        #[source]
        source: ArtifactTextError,
    },
    #[error("missing ids: {ids:?}")]
    Missing { ids: Vec<String> },
}

/// Every actual request retains the complete original chapter and the complete inventory.
pub(super) struct ArtifactRequest<'a> {
    pub chapter: &'a ocr::Chapter,
    pub source: &'a ocr::content::ArtifactSource,
    pub targets: &'a [ArtifactTarget],
    pub feedback: &'a str,
}

impl ArtifactRequest<'_> {
    pub fn input(&self) -> String {
        let chapter = chapter_context(self.chapter);
        serde_json::json!({"chapter":chapter,"manifest":self.source.manifest,"targets":self.targets,"validation_feedback":self.feedback}).to_string()
    }
}

fn chapter_context(chapter: &ocr::Chapter) -> String {
    let mut text = format!(
        "Document: {}\nName: {:?}\n",
        chapter.index.media_title, chapter.index.name
    );
    for piece in &chapter.pieces {
        let detail = serde_json::to_string(&piece.detail).expect("piece details always serialize");
        text.push_str(&format!(
            "\nPage {} (printed {:?}), piece {}\n{}\n{}\n",
            piece.id.page, piece.printed_page_number, piece.id.number, detail, piece.content
        ));
    }
    text
}
