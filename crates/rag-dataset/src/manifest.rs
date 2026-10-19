//! The record of what one build made: the options it ran with, the documents and files it holds
//! with their hashes and counts, and what the models cost.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rag_core::{DocId, UsageTally};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::documents::{DocRole, Split};
use crate::layout::{DatasetFolder, WriteError, write_json};
use crate::options::BuildOptions;
use crate::sample::PIPELINE_VERSION;

#[derive(Debug, Clone, Serialize)]
pub struct Manifest {
    pub name: String,
    /// Seconds since 1970-01-01 UTC.
    pub created_at: u64,
    pub pipeline_version: String,
    pub options: BuildOptions,
    pub documents: Vec<ManifestDocument>,
    pub files: Vec<ManifestFile>,
    pub usage: Vec<ModelUsageLine>,
    /// `None` when a model has no price.
    pub cost_usd: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ManifestDocument {
    pub document_id: DocId,
    pub title: String,
    pub doc_role: DocRole,
    pub split: Split,
    /// The sections that were kept.
    pub sections: usize,
    pub n_tokens: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ManifestFile {
    /// Relative to the dataset folder, with `/` between the names.
    pub path: String,
    /// Hex of the SHA-256 of the file's bytes.
    pub sha256: String,
    /// The lines of the file.
    pub samples: usize,
    /// The sum of the `n_tokens` of its lines. Zero for a file whose lines have none.
    pub n_tokens: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelUsageLine {
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub estimated: bool,
}

#[derive(thiserror::Error, Debug)]
pub enum ManifestError {
    #[error(
        "could not read {} to list it in the manifest; check that the dataset folder can be read",
        path.display()
    )]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error(transparent)]
    Write(#[from] WriteError),
}

impl Manifest {
    /// The manifest of a dataset whose files are written. It lists every file of records that
    /// exists in the folder.
    ///
    /// # Errors
    /// [`ManifestError::Read`] when a file of the dataset cannot be read.
    pub fn of(
        options: &BuildOptions,
        folder: &DatasetFolder,
        documents: Vec<ManifestDocument>,
        usage: &UsageTally,
    ) -> Result<Manifest, ManifestError> {
        let mut files = Vec::new();
        for path in folder.jsonl_files() {
            if path.is_file() {
                files.push(ManifestFile::read(folder.root(), &path)?);
            }
        }
        Ok(Manifest {
            name: options.name.clone(),
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |since| since.as_secs()),
            pipeline_version: PIPELINE_VERSION.to_owned(),
            options: options.clone(),
            documents,
            files,
            usage: usage
                .models()
                .map(|(model, used)| ModelUsageLine {
                    model: model.to_owned(),
                    input_tokens: used.tokens.input_tokens,
                    output_tokens: used.tokens.output_tokens,
                    cache_read_tokens: used.tokens.cache_read_tokens,
                    cache_write_tokens: used.tokens.cache_write_tokens,
                    estimated: used.estimated,
                })
                .collect(),
            cost_usd: usage.cost_usd(),
        })
    }

    /// # Errors
    /// [`WriteError`] when the manifest cannot be written.
    pub fn write(&self, folder: &DatasetFolder) -> Result<(), WriteError> {
        write_json(&folder.manifest(), self)
    }
}

impl ManifestFile {
    fn read(root: &Path, path: &Path) -> Result<ManifestFile, ManifestError> {
        let bytes = fs::read(path).map_err(|source| ManifestError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let relative = path.strip_prefix(root).unwrap_or(path);
        let names: Vec<_> = relative
            .components()
            .map(|part| part.as_os_str().to_string_lossy())
            .collect();
        let text = String::from_utf8_lossy(&bytes);
        let lines: Vec<&str> = text.lines().collect();
        Ok(ManifestFile {
            path: names.join("/"),
            sha256: Sha256::digest(&bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
            samples: lines.len(),
            n_tokens: lines.iter().map(|line| tokens_of_line(line)).sum(),
        })
    }
}

fn tokens_of_line(line: &str) -> usize {
    serde_json::from_str::<Value>(line)
        .ok()
        .and_then(|record| record.get("n_tokens")?.as_u64())
        .map_or(0, |tokens| tokens as usize)
}
