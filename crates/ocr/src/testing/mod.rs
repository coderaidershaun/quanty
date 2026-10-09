//! Support for tests, behind the cargo feature `testing`: stand-ins for the two paid services, so
//! the whole chapter run can be tried offline, and small helpers for the sample chapter.

mod pages;
mod stub;

use std::path::{Path, PathBuf};

use crate::ChapterJob;
use crate::content::{MediaDocument, parse_chapter_file_name};

pub use stub::{Call, Scenario, StubServices};

pub fn sample_pdf() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/chapter-1-sample-pages.pdf")
}

pub fn sample_job(output_root: &Path) -> ChapterJob {
    let document = MediaDocument {
        media_title: "Option Volatility and Pricing".to_owned(),
        name: parse_chapter_file_name("chapter-1-sample-pages.pdf").unwrap(),
    };
    ChapterJob::new(document, &sample_pdf(), output_root).unwrap()
}

pub fn page_folder(chapter: &Path, position: u32) -> PathBuf {
    chapter.join(format!("page-num-{position}"))
}

pub fn read_json(path: &Path) -> serde_json::Value {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("reading {}: {error}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}
