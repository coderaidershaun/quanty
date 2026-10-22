//! Saved descriptions remain independent of conversion, so readers can use accepted work without a model.

use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::ContentError;
use super::index::{read_json, write_json};

/// Independently versioned metadata file.
pub const ARTIFACTS_FILE: &str = "artifacts.json";
/// Supported saved metadata shape.
pub const ARTIFACTS_VERSION: u32 = 1;

/// A source position, independent of retrieval item numbering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactPiece {
    pub page: u32,
    pub number: u32,
}

/// One complete artifact, including every part of a listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactTarget {
    pub id: String,
    pub kind: String,
    pub pieces: Vec<ArtifactPiece>,
}

/// Source identity and ordered inventory, built by the chapter reader.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactSource {
    pub signature: String,
    pub manifest: Vec<ArtifactTarget>,
}

/// Display text from one accepted answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactText {
    pub id: String,
    pub title: String,
    pub description: String,
}

impl ArtifactText {
    /// Rejects multiline text, formatting and values outside the display limits.
    pub fn is_valid(&self) -> bool {
        self.validate().is_ok()
    }

    /// Technical identifiers and punctuation are literal plain text.
    ///
    /// # Errors
    /// Identifies the first invalid field with a reason and bounded escaped evidence.
    pub fn validate(&self) -> Result<(), ArtifactTextError> {
        validate_field("title", &self.title, 80)?;
        validate_field("description", &self.description, 280)
    }
}

/// A failed display constraint with at most 96 escaped excerpt characters plus an ellipsis.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{field}: {reason}; value=\"{excerpt}\"")]
pub struct ArtifactTextError {
    field: &'static str,
    reason: TextReason,
    excerpt: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
enum TextReason {
    #[error("empty or whitespace-only text; supply source-grounded text")]
    Empty,
    #[error("leading or trailing whitespace; trim the field")]
    Whitespace,
    #[error("{actual} characters exceeds {limit}; shorten the field")]
    Length { actual: usize, limit: usize },
    #[error("line or paragraph break at character {position}; use one line")]
    Break { position: usize },
    #[error("control character at character {position}; remove it")]
    Control { position: usize },
    #[error("Markdown formatting; remove backticks, emphasis or heading/list markers")]
    Formatting,
}

fn validate_field(field: &'static str, text: &str, limit: usize) -> Result<(), ArtifactTextError> {
    let actual = text.chars().count();
    let reason = if text.trim().is_empty() {
        Some(TextReason::Empty)
    } else if text != text.trim() {
        Some(TextReason::Whitespace)
    } else if actual > limit {
        Some(TextReason::Length { actual, limit })
    } else if let Some(position) = text
        .chars()
        .position(|c| matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}'))
    {
        Some(TextReason::Break {
            position: position + 1,
        })
    } else if let Some(position) = text.chars().position(char::is_control) {
        Some(TextReason::Control {
            position: position + 1,
        })
    } else if has_formatting(text) {
        Some(TextReason::Formatting)
    } else {
        None
    };
    match reason {
        Some(reason) => Err(ArtifactTextError {
            field,
            reason,
            excerpt: excerpt(text),
        }),
        None => Ok(()),
    }
}

fn excerpt(text: &str) -> String {
    let mut excerpt = String::new();
    let mut count = 0;
    for character in text.chars() {
        let escaped = character.escape_debug().to_string();
        let width = escaped.chars().count();
        if count + width > 96 {
            excerpt.push('…');
            break;
        }
        excerpt.push_str(&escaped);
        count += width;
    }
    excerpt
}

fn has_formatting(text: &str) -> bool {
    let prefix = if text.starts_with('#') {
        text.trim_start_matches('#')
    } else if text.starts_with(['*', '-', '+']) {
        &text[1..]
    } else {
        ""
    };
    if text.contains('`') || prefix.starts_with(char::is_whitespace) {
        return true;
    }
    for (start, _) in text.match_indices('*') {
        let before = text[..start].chars().next_back();
        if before.is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '*')) {
            continue;
        }
        let width = text[start..].bytes().take_while(|&b| b == b'*').count();
        let rest = &text[start + width..];
        if rest.starts_with(char::is_whitespace) || rest.is_empty() {
            continue;
        }
        let delimiter = &text[start..start + width];
        for (end, _) in rest.match_indices(delimiter) {
            let content = &rest[..end];
            let after = rest[end + width..].chars().next();
            if !content.is_empty()
                && !content.ends_with(|c: char| c.is_whitespace() || c == '*')
                && after.is_none_or(|c| !c.is_alphanumeric() && !matches!(c, '_' | '*'))
            {
                return true;
            }
        }
    }
    false
}

/// Provenance belongs to each entry, because resumed batches can use different models.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactEntry {
    #[serde(flatten)]
    pub text: ArtifactText,
    pub model: String,
    pub cache_identity: String,
}

/// Accepted batches, including partial work and the policy that produced them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChapterArtifacts {
    pub format_version: u32,
    pub source: ArtifactSource,
    pub generation_signature: String,
    pub entries: Vec<ArtifactEntry>,
}

impl ChapterArtifacts {
    /// Missing files return `None`.
    ///
    /// # Errors
    /// Returns a content error for unreadable or malformed files.
    pub fn read(folder: &Path) -> Result<Option<Self>, ContentError> {
        let path = folder.join(ARTIFACTS_FILE);
        if !path.exists() {
            return Ok(None);
        }
        read_json(&path).map(Some)
    }

    /// Atomically replaces the checkpoint with all accepted entries.
    ///
    /// # Errors
    /// Returns a content error when the checkpoint cannot be written.
    pub fn write(&self, folder: &Path) -> Result<(), ContentError> {
        write_json(&folder.join(ARTIFACTS_FILE), self)
    }

    /// Display validity deliberately ignores policy age and incomplete coverage.
    pub fn matches_source(&self, source: &ArtifactSource) -> bool {
        if self.format_version != ARTIFACTS_VERSION || &self.source != source {
            return false;
        }
        let known: BTreeSet<_> = source
            .manifest
            .iter()
            .map(|target| target.id.as_str())
            .collect();
        if known.len() != source.manifest.len() {
            return false;
        }
        let mut seen = BTreeSet::new();
        self.entries.iter().all(|entry| {
            entry.text.is_valid()
                && known.contains(entry.text.id.as_str())
                && seen.insert(entry.text.id.as_str())
        })
    }

    /// Looks up saved text by source identity. Call `matches_source` before displaying it.
    pub fn text(&self, id: &str) -> Option<&ArtifactText> {
        self.entries
            .iter()
            .find(|entry| entry.text.id == id)
            .map(|entry| &entry.text)
    }
}
