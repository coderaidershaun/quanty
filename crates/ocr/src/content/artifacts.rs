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
        plain(&self.title, 80) && plain(&self.description, 280)
    }
}

fn plain(text: &str, limit: usize) -> bool {
    !text.trim().is_empty()
        && text == text.trim()
        && text.chars().count() <= limit
        && !text
            .chars()
            .any(|character| character.is_control() || matches!(character, '\u{2028}' | '\u{2029}'))
        && !text.contains('`')
        && !text.contains("**")
        && !text.split('<').skip(1).any(|tail| {
            tail.starts_with(|character: char| character.is_ascii_alphabetic() || character == '/')
                && tail.contains('>')
        })
        && !text.contains("__")
        && !text.contains("](")
        && !text.starts_with(['#', '*'])
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
