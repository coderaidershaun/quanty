//! The file that every resolution decision is added to, as one line of JSON for each decision, so
//! that a person can see why a name was linked to a concept or made a new one.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use graph::ConceptNode;
use rag_core::{ConceptHit, ConceptId, ItemId};
use serde::Serialize;

use super::ConceptError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Rule {
    /// A stored concept has the same normalised name or alias.
    ExactName,
    /// The nearest stored concept scored at or over the high threshold.
    HighScore,
    /// The score was between the thresholds and the model said the two are the same concept.
    LlmSame,
    /// The score was between the thresholds and the model said they are different.
    LlmDifferent,
    /// The nearest stored concept scored under the low threshold.
    LowScore,
    /// No concept is stored yet.
    NothingStored,
}

#[derive(Serialize)]
struct ConceptRef<'a> {
    id: ConceptId,
    name: &'a str,
}

/// One decision. A field that does not apply to its rule is left out of the line.
#[derive(Serialize)]
pub(super) struct Decision<'a> {
    item: ItemId,
    /// The name as the item wrote it.
    name: &'a str,
    rule: Rule,
    /// The stored concept that the name was linked to.
    #[serde(skip_serializing_if = "Option::is_none")]
    matched: Option<ConceptRef<'a>>,
    /// The nearest stored concept, which was turned down for a new concept.
    #[serde(skip_serializing_if = "Option::is_none")]
    nearest: Option<ConceptRef<'a>>,
    /// The cosine score of the nearest stored concept.
    #[serde(skip_serializing_if = "Option::is_none")]
    score: Option<f32>,
    /// The new concept.
    #[serde(skip_serializing_if = "Option::is_none")]
    created: Option<ConceptId>,
}

/// The name that an item used for a concept. Every decision is about one of these.
#[derive(Debug, Clone, Copy)]
pub(super) struct Naming<'a> {
    pub item: ItemId,
    /// The name as the item wrote it.
    pub name: &'a str,
}

impl<'a> Naming<'a> {
    /// The name was linked to `matched`. `score` is there when the rule searched for a concept.
    pub(super) fn linked(
        self,
        rule: Rule,
        matched: &'a ConceptNode,
        score: Option<f32>,
    ) -> Decision<'a> {
        Decision {
            item: self.item,
            name: self.name,
            rule,
            matched: Some(ConceptRef {
                id: matched.id,
                name: &matched.name,
            }),
            nearest: None,
            score,
            created: None,
        }
    }

    /// The name became the concept `created`. `nearest` is there when a stored concept was
    /// found and turned down.
    pub(super) fn created(
        self,
        rule: Rule,
        created: ConceptId,
        nearest: Option<&'a ConceptHit>,
    ) -> Decision<'a> {
        Decision {
            item: self.item,
            name: self.name,
            rule,
            matched: None,
            nearest: nearest.map(|hit| ConceptRef {
                id: hit.id,
                name: &hit.name,
            }),
            score: nearest.map(|hit| hit.score),
            created: Some(created),
        }
    }
}

pub(super) struct DecisionLog {
    path: PathBuf,
    file: File,
}

impl DecisionLog {
    /// Opens the file to add lines to its end. The file and its folder are made when missing.
    ///
    /// # Errors
    /// [`ConceptError::DecisionLog`] when they cannot be made or opened.
    pub(super) fn open(path: &Path) -> Result<DecisionLog, ConceptError> {
        let log_error = |source| ConceptError::DecisionLog {
            path: path.to_path_buf(),
            source,
        };
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder).map_err(log_error)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(log_error)?;
        Ok(DecisionLog {
            path: path.to_path_buf(),
            file,
        })
    }

    /// Adds one line. A decision is added after the writes it led to have succeeded.
    ///
    /// # Errors
    /// [`ConceptError::DecisionLog`] when the line cannot be written.
    pub(super) fn append(&self, decision: &Decision<'_>) -> Result<(), ConceptError> {
        let mut line = serde_json::to_vec(decision)
            .map_err(std::io::Error::other)
            .map_err(|source| ConceptError::DecisionLog {
                path: self.path.clone(),
                source,
            })?;
        line.push(b'\n');
        // One write for the whole line, so that a line is never split in two.
        (&self.file)
            .write_all(&line)
            .map_err(|source| ConceptError::DecisionLog {
                path: self.path.clone(),
                source,
            })
    }
}
