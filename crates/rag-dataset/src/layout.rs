//! The files of one dataset, where each lives, and the one way every stage writes them: whole or
//! not at all.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// The folder of one dataset. Every path a build reads or writes comes from here.
#[derive(Debug, Clone)]
pub struct DatasetFolder {
    root: PathBuf,
}

impl DatasetFolder {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        DatasetFolder { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The training sections as the book prints them.
    pub fn cpt_raw(&self) -> PathBuf {
        self.root.join("cpt/raw.jsonl")
    }

    /// The sections of the documents that are held out of training.
    pub fn cpt_heldout(&self) -> PathBuf {
        self.root.join("cpt/heldout.jsonl")
    }

    /// The sections that the checks kept out, each with its reason.
    pub fn cpt_rejected(&self) -> PathBuf {
        self.root.join("cpt/rejected.jsonl")
    }

    pub fn cpt_synthetic(&self) -> PathBuf {
        self.root.join("cpt/synthetic.jsonl")
    }

    pub fn sft_train(&self) -> PathBuf {
        self.root.join("sft/train.jsonl")
    }

    pub fn sft_heldout(&self) -> PathBuf {
        self.root.join("sft/heldout.jsonl")
    }

    pub fn sft_rejected(&self) -> PathBuf {
        self.root.join("sft/rejected.jsonl")
    }

    pub fn plan(&self) -> PathBuf {
        self.root.join("plan.json")
    }

    pub fn manifest(&self) -> PathBuf {
        self.root.join("manifest.json")
    }

    /// Every file of records, in the order the manifest lists them.
    pub fn jsonl_files(&self) -> [PathBuf; 7] {
        [
            self.cpt_raw(),
            self.cpt_heldout(),
            self.cpt_rejected(),
            self.cpt_synthetic(),
            self.sft_train(),
            self.sft_heldout(),
            self.sft_rejected(),
        ]
    }
}

#[derive(thiserror::Error, Debug)]
#[error("could not write {}; check that the dataset folder can be written to", path.display())]
pub struct WriteError {
    pub path: PathBuf,
    #[source]
    pub source: io::Error,
}

/// One line for each record. The file is written beside its name and then renamed, so a run that
/// is killed leaves no half-written file. The folder is made when it is missing.
///
/// # Errors
/// [`WriteError`] when the folder or the file cannot be written, or a record cannot be made into
/// JSON.
pub fn write_jsonl<T: Serialize>(path: &Path, records: &[T]) -> Result<(), WriteError> {
    let mut text = String::new();
    for record in records {
        let line = serde_json::to_string(record).map_err(|error| write_error(path, error))?;
        text.push_str(&line);
        text.push('\n');
    }
    write_whole(path, &text)
}

/// Pretty JSON with a closing newline, written the way [`write_jsonl`] writes.
///
/// # Errors
/// [`WriteError`] when the folder or the file cannot be written, or the value cannot be made into
/// JSON.
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), WriteError> {
    let mut text = serde_json::to_string_pretty(value).map_err(|error| write_error(path, error))?;
    text.push('\n');
    write_whole(path, &text)
}

fn write_error(path: &Path, error: serde_json::Error) -> WriteError {
    WriteError {
        path: path.to_path_buf(),
        source: io::Error::other(error),
    }
}

fn write_whole(path: &Path, text: &str) -> Result<(), WriteError> {
    let failed = |source| WriteError {
        path: path.to_path_buf(),
        source,
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(failed)?;
    }
    let mut temporary: OsString = path.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    fs::write(&temporary, text).map_err(failed)?;
    fs::rename(&temporary, path).map_err(failed)
}
