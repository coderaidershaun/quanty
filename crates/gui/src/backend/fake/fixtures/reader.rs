//! The saved chapter files as the fake reads them, and the ways a sample cannot be read.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use uuid::Uuid;

use crate::contract::DocumentName;

#[derive(Debug, thiserror::Error)]
pub(in crate::backend::fake) enum SampleError {
    #[error("no sample document has the id {doc}")]
    UnknownDocument { doc: Uuid },
    #[error("the sample document has {page_count} pages, so page {page} is not in it")]
    NoSuchPage { page: u32, page_count: u32 },
    #[error("page {page} of the sample document {doc} has no piece {piece}")]
    NoSuchPiece { doc: Uuid, page: u32, piece: u32 },
    #[error("could not read {}: {source}", path.display())]
    Unreadable {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{} is not a sample file of the expected shape: {source}", path.display())]
    Malformed {
        path: PathBuf,
        source: serde_json::Error,
    },
}

// SMELL: the converter keeps its own reader of these files, and the fake may not name that crate,
// so a change to the saved format must be made in both.
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) struct ChapterFile {
    pub(super) media_title: String,
    name: NameFile,
    pub(super) page_count: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum NameFile {
    Chapter { number: u32, name: String },
    Title(String),
}

impl ChapterFile {
    pub(super) fn name(&self) -> DocumentName {
        match &self.name {
            NameFile::Chapter { number, name } => DocumentName::Chapter {
                number: *number,
                name: name.clone(),
            },
            NameFile::Title(title) => DocumentName::Title(title.clone()),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) struct PageFile {
    pub(super) printed_page_number: Option<String>,
    pub(super) pieces: Vec<PieceFile>,
}

#[derive(Deserialize)]
pub(super) struct PieceFile {
    pub(super) number: u32,
    pub(super) file: String,
    #[serde(flatten)]
    pub(super) detail: Detail,
}

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "kebab-case"
)]
pub(super) enum Detail {
    Heading {
        rank: u8,
        #[serde(default)]
        printed_number: Option<String>,
    },
    Text,
    Formula {
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        name: Option<String>,
    },
    Figure {
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        caption: Option<String>,
        #[serde(default)]
        image: Option<FigureImage>,
    },
    Table {
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        caption: Option<String>,
    },
    Footnote {
        #[serde(default)]
        marker: Option<String>,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) struct FigureImage {
    pub(super) file: String,
    pub(super) shows: Shows,
    #[serde(default)]
    pub(super) cut: Option<Cut>,
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Shows {
    Figure,
    WholePage,
}

#[derive(Deserialize)]
pub(super) struct Cut {
    pub(super) left: i32,
    pub(super) top: i32,
    pub(super) right: i32,
    pub(super) bottom: i32,
}

pub(super) fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, SampleError> {
    let text = std::fs::read_to_string(path).map_err(|source| SampleError::Unreadable {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| SampleError::Malformed {
        path: path.to_path_buf(),
        source,
    })
}

pub(super) fn read_piece(path: &Path) -> Result<String, SampleError> {
    let mut text = std::fs::read_to_string(path).map_err(|source| SampleError::Unreadable {
        path: path.to_path_buf(),
        source,
    })?;
    if text.ends_with('\n') {
        text.pop();
    }
    Ok(text)
}

pub(super) fn read_page_file(folder: &Path, position: u32) -> Result<PageFile, SampleError> {
    read_json(
        &folder
            .join(format!("page-num-{position}"))
            .join("page.json"),
    )
}
