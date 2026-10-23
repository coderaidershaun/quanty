//! Only the live backend may reach stores, so fixtures remain isolated from them.

use std::fs;
use std::path::{Path, PathBuf};

/// Every `.rs` file under `folder`, in any order.
fn rust_files(folder: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![folder.to_path_buf()];
    while let Some(next) = pending.pop() {
        let entries = fs::read_dir(&next)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", next.display()));
        for entry in entries {
            let path = entry.expect("a folder entry can be read").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                found.push(path);
            }
        }
    }
    found
}

fn manifest_folder() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

/// True when `path_start` stands in `line` as the start of a path: the character before it is
/// not part of a longer name.
fn names_path(line: &str, path_start: &str) -> bool {
    line.match_indices(path_start).any(|(at, _)| {
        line[..at]
            .chars()
            .next_back()
            .is_none_or(|before| !(before.is_alphanumeric() || before == '_' || before == ':'))
    })
}

#[test]
fn only_live_names_store_crates() {
    let source = manifest_folder().join("src");
    let live_backend = source.join("live");
    let crates = [
        "graph::",
        "ocr::",
        "rag_core::",
        "rag_dataset::",
        "rag_ingestion::",
        "rag_retrieval::",
    ];
    let mut breaks = Vec::new();
    for file in rust_files(&source) {
        if file.starts_with(&live_backend) {
            continue;
        }
        for (number, line) in read(&file).lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            for name in crates {
                if names_path(line, name) {
                    breaks.push(format!(
                        "{}:{}: {}",
                        file.display(),
                        number + 1,
                        line.trim()
                    ));
                }
            }
        }
    }
    assert!(
        breaks.is_empty(),
        "only src/live may name a backend crate:\n{}",
        breaks.join("\n")
    );
}
