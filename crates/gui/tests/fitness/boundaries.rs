//! Checks the two rules that keep the desktop app apart from the stores: only the live backend
//! and `main` name a backend crate, and only `main` builds the settings.
//!
//! These are fitness tests in a weak sense: an import or a settings call that breaks a rule
//! compiles and works, and only shows when a fixture or a test reaches a real store.

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
fn only_the_live_backend_and_main_name_a_backend_crate() {
    let source = manifest_folder().join("src");
    let live_backend = source.join("backend").join("live");
    let main = source.join("main.rs");
    let crates = [
        "graph::",
        "ocr::",
        "rag_core::",
        "rag_ingestion::",
        "rag_retrieval::",
    ];
    let mut breaks = Vec::new();
    for file in rust_files(&source) {
        if file == main || file.starts_with(&live_backend) {
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
        "only src/backend/live and src/main.rs may name a backend crate:\n{}",
        breaks.join("\n")
    );
}

#[test]
fn only_main_builds_the_settings() {
    let manifest = manifest_folder();
    let main = manifest.join("src").join("main.rs");
    // Written in two parts so that this file does not find itself.
    let calls = [
        concat!("Config::", "load"),
        concat!("Config::", "from_sources"),
    ];
    let mut breaks = Vec::new();
    for folder in [manifest.join("src"), manifest.join("tests")] {
        for file in rust_files(&folder) {
            if file == main {
                continue;
            }
            for (number, line) in read(&file).lines().enumerate() {
                if calls.iter().any(|call| line.contains(call)) {
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
        "only src/main.rs may build the settings:\n{}",
        breaks.join("\n")
    );
}
