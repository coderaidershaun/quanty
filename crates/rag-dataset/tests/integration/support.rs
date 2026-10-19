//! Shared by the tests of this crate: the sample documents, the stand-in `claude`, and the reads
//! of what a build wrote.

use std::fs;
use std::path::{Path, PathBuf};

use ocr::Catalogue;
use rag_core::Config;
use rag_dataset::BuildOptions;
use rag_dataset::documents::{ExportDocument, exported_documents};
use serde_json::Value;

/// The converted chapters that are kept in the repository, made absolute the way a build makes
/// its content folder.
pub fn sample_content() -> PathBuf {
    std::path::absolute(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/content"))
        .expect("the sample content folder should have a full path")
}

/// Seven pages converted from a real book: every kind of piece, headings, footnotes and page
/// breaks.
pub fn sample_chapter() -> PathBuf {
    sample_content().join("option-volatility-and-pricing/chapter-1")
}

/// Written by hand: options pricing at the level of intuition.
pub fn intuition_chapter() -> PathBuf {
    sample_content().join("quanty-sample-notes/chapter-1")
}

/// Written by hand: the derivation and the formulas of the Black–Scholes model, and one Python
/// listing.
pub fn in_depth_chapter() -> PathBuf {
    sample_content().join("quanty-sample-notes/chapter-2")
}

pub fn sample_chapters() -> [PathBuf; 3] {
    [sample_chapter(), intuition_chapter(), in_depth_chapter()]
}

/// The folder that holds the stand-in `claude`, which asks no model and records that it ran.
pub fn stand_in_claude_folder() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/stand-in-claude")
}

/// The settings of a build with every default, over this content folder, writing into `out`.
/// Nothing is read from the environment or from a `.env`, so no test can reach a real folder.
pub fn options_over(content_folder: &Path, out: &Path) -> BuildOptions {
    let config = Config::from_sources(|_| None, None).expect("the default settings should load");
    BuildOptions {
        content_folder: content_folder.to_path_buf(),
        cache_folder: out.join("cache"),
        out: out.to_path_buf(),
        ..BuildOptions::new("t", &config)
    }
}

/// The documents a build over this content folder exports, with every default.
pub fn exported_over(content_folder: &Path) -> Vec<ExportDocument> {
    let catalogue = Catalogue::read(content_folder).expect("the catalogue should be read");
    exported_documents(
        &catalogue,
        &options_over(content_folder, Path::new("unused")),
    )
    .expect("the selection should hold")
}

/// The document whose folder is this one.
pub fn exported_at(content_folder: &Path, chapter_folder: &Path) -> ExportDocument {
    exported_over(content_folder)
        .into_iter()
        .find(|document| document.folder == chapter_folder)
        .expect("the chapter folder should be one of the exported documents")
}

/// Every line of a file of records, read back as JSON.
pub fn read_jsonl(path: &Path) -> Vec<Value> {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{} should be readable: {error}", path.display()))
        .lines()
        .map(|line| serde_json::from_str(line).expect("a line of records should be JSON"))
        .collect()
}

/// Copies a folder and everything in it.
pub fn copy_folder(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("the copy's folder should be made");
    for entry in fs::read_dir(from).expect("the folder to copy should be readable") {
        let entry = entry.expect("an entry of the folder should be readable");
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_folder(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).expect("a file should be copied");
        }
    }
}
