//! The folder of kept answers: one plain file for each question that was answered well, so that
//! the same question is never paid for twice.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rag_core::Question;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::ConceptError;
use super::question::Extraction;

const FILE_EXTENSION: &str = "json";

pub(super) struct Cache {
    folder: PathBuf,
}

/// The name that an answer is kept under. It is made from the prompt version, the model and
/// everything that is sent, so a person who edits the prompt or the schema and forgets to raise
/// the version still gets fresh answers.
pub(super) fn key_of(prompt_version: &str, model: &str, question: Question<'_>) -> String {
    let mut hasher = Sha256::new();
    for part in [
        prompt_version,
        model,
        question.system_prompt,
        question.schema,
        question.input,
    ] {
        // The length goes first, so that the end of one part cannot be taken for the start of the
        // next.
        hasher.update(part.len().to_le_bytes());
        hasher.update(part.as_bytes());
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

impl Cache {
    /// Makes the folder when it is missing.
    ///
    /// # Errors
    /// [`ConceptError::Cache`] when the folder cannot be made.
    pub(super) fn open(folder: &Path) -> Result<Cache, ConceptError> {
        fs::create_dir_all(folder).map_err(|source| ConceptError::Cache {
            path: folder.to_path_buf(),
            source,
        })?;
        Ok(Cache {
            folder: folder.to_path_buf(),
        })
    }

    fn path_of(&self, key: &str) -> PathBuf {
        self.folder.join(key).with_extension(FILE_EXTENSION)
    }

    /// The kept answer, or `None` when there is none. A file that is not JSON, or does not fit
    /// the reply, counts as no answer: the next good answer overwrites it.
    ///
    /// # Errors
    /// [`ConceptError::Cache`] when the file exists but cannot be read.
    pub(super) fn get(&self, key: &str) -> Result<Option<Extraction>, ConceptError> {
        let path = self.path_of(key);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(ConceptError::Cache { path, source }),
        };
        let extraction = serde_json::from_slice::<Value>(&bytes)
            .ok()
            .and_then(|value| Extraction::from_value(&value).ok());
        Ok(extraction)
    }

    /// Keeps the answer exactly as the model gave it. It is written under a temporary name and
    /// then renamed, so a run that is killed leaves no half-written file.
    ///
    /// # Errors
    /// [`ConceptError::Cache`] when the file cannot be written.
    pub(super) fn put(&self, key: &str, answer: &Value) -> Result<(), ConceptError> {
        let path = self.path_of(key);
        let temporary = self
            .folder
            .join(format!("{key}.{}.tmp", std::process::id()));
        let written = serde_json::to_vec(answer)
            .map_err(io::Error::other)
            .and_then(|bytes| fs::write(&temporary, bytes))
            .and_then(|()| fs::rename(&temporary, &path));
        written.map_err(|source| ConceptError::Cache { path, source })
    }
}
