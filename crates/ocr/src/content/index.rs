//! The three index files, `chapter.json`, `page.json` and `image.json`: what each holds and how it
//! is read and written.

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::conversion::{CallRecord, Conversion};
use super::piece::{PieceEntry, Relationship};
use super::{CHAPTER_INDEX_FILE, ContentError, IMAGE_INDEX_FILE, PAGE_INDEX_FILE};

/// What `chapter.json` holds. `finished` is the last thing written, so a half-converted chapter
/// is never mistaken for a whole one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ChapterIndex {
    pub format_version: u32,
    pub book_title: String,
    pub chapter_number: u32,
    pub chapter_name: String,
    pub source_file: String,
    pub source_sha256: String,
    pub page_count: u32,
    pub finished: bool,
}

/// A hand-written page may leave out everything marked `default`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PageIndex {
    pub format_version: u32,
    /// 1 for the first page of the chapter PDF.
    pub page_position: u32,
    /// As printed, so possibly "xii". `None` when the page shows no number.
    #[serde(default)]
    pub printed_page_number: Option<String>,
    #[serde(default)]
    pub running_header: Option<String>,
    #[serde(default)]
    pub starts_mid_sentence: bool,
    #[serde(default)]
    pub ends_mid_sentence: bool,
    pub pieces: Vec<PieceEntry>,
    #[serde(default)]
    pub relationships: Vec<Relationship>,
    #[serde(default)]
    pub conversion: Option<Conversion>,
}

/// What `image.json` holds: which picture the folder was made from and what was read from it. It
/// is the last file written, so a half-converted picture is never mistaken for a whole one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ImageIndex {
    pub format_version: u32,
    /// The file name the picture had.
    pub source_file: String,
    pub source_sha256: String,
    /// The name of the copy of the picture, in the same folder.
    pub picture: String,
    pub label: Option<String>,
    pub caption: Option<String>,
    pub printed_text: Vec<String>,
    pub calls: Vec<CallRecord>,
}

impl ChapterIndex {
    pub fn read(chapter_folder: &Path) -> Result<Self, ContentError> {
        read_json(&chapter_folder.join(CHAPTER_INDEX_FILE))
    }

    /// Writes the file whole or not at all, so a killed run never leaves half of it.
    pub fn write(&self, chapter_folder: &Path) -> Result<(), ContentError> {
        write_json(&chapter_folder.join(CHAPTER_INDEX_FILE), self)
    }
}

impl PageIndex {
    pub fn read(page_folder: &Path) -> Result<Self, ContentError> {
        read_json(&page_folder.join(PAGE_INDEX_FILE))
    }

    pub fn write(&self, page_folder: &Path) -> Result<(), ContentError> {
        write_json(&page_folder.join(PAGE_INDEX_FILE), self)
    }
}

impl ImageIndex {
    pub fn read(image_folder: &Path) -> Result<Self, ContentError> {
        read_json(&image_folder.join(IMAGE_INDEX_FILE))
    }

    pub fn write(&self, image_folder: &Path) -> Result<(), ContentError> {
        write_json(&image_folder.join(IMAGE_INDEX_FILE), self)
    }
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, ContentError> {
    let text = std::fs::read_to_string(path).map_err(|source| ContentError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| ContentError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

/// Writes pretty json to a neighbouring `.tmp` file and renames it into place.
fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), ContentError> {
    let write_error = |source| ContentError::Write {
        path: path.to_path_buf(),
        source,
    };
    let mut text =
        serde_json::to_string_pretty(value).map_err(|error| write_error(error.into()))?;
    text.push('\n');
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    std::fs::write(&temporary, text).map_err(write_error)?;
    std::fs::rename(&temporary, path).map_err(write_error)
}
