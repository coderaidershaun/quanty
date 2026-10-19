//! The `rag-dataset` command, started the way a person starts it.

use std::fs;
use std::process::Command;

use ocr::read_chapter;
use rag_dataset::section::sections_of;
use rag_ingestion::chapter_items;
use serde_json::Value;

use crate::support;

#[test]
fn a_dry_run_exports_every_sample_section_and_asks_no_model() {
    let work = tempfile::tempdir().unwrap();
    let stand_in_work = work.path().join("claude");
    fs::create_dir_all(&stand_in_work).unwrap();
    let path = format!(
        "{}:{}",
        support::stand_in_claude_folder().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let dataset = work.path().join("t");

    // The command runs in a folder with no `.env` and no saved model above it, and every folder
    // it could write to is under the temporary folder.
    let output = Command::new(env!("CARGO_BIN_EXE_rag-dataset"))
        .args(["build", "--name", "t", "--out"])
        .arg(&dataset)
        .args(["--no-graph", "--dry-run"])
        .current_dir(work.path())
        .env("CONTENT_DIR", support::sample_content())
        .env("DATASET_DIR", work.path().join("datasets"))
        .env("DATASET_CACHE_DIR", work.path().join("dataset-cache"))
        .env("LLM_PROVIDER", "claude")
        .env("LLM_MODEL", "sonnet")
        .env("PATH", path)
        .env("STAND_IN_CLAUDE", &stand_in_work)
        .output()
        .expect("the rag-dataset binary should start");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");

    let mut expected_ids = Vec::new();
    let documents = support::exported_over(&support::sample_content());
    assert_eq!(documents.len(), 3);
    for document in &documents {
        let chapter = read_chapter(&document.folder).unwrap();
        for section in sections_of(document, &chapter, &chapter_items(&chapter)) {
            expected_ids.push(format!("{}:raw", section.id));
        }
    }
    let raw = support::read_jsonl(&dataset.join("cpt/raw.jsonl"));
    let ids: Vec<&str> = raw
        .iter()
        .map(|record| record["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, expected_ids);
    for record in &raw {
        assert_eq!(record["kind"], "raw");
        assert_eq!(record["split"], "train");
        assert_eq!(record["tokens_estimated"], true);
        assert_eq!(record["pipeline_version"], env!("CARGO_PKG_VERSION"));
    }
    for empty in ["cpt/heldout.jsonl", "cpt/rejected.jsonl"] {
        assert_eq!(
            fs::read_to_string(dataset.join(empty)).unwrap(),
            "",
            "{empty}"
        );
    }

    let plan: Value =
        serde_json::from_str(&fs::read_to_string(dataset.join("plan.json")).unwrap()).unwrap();
    assert_eq!(
        plan["sections"].as_array().unwrap().len(),
        expected_ids.len()
    );
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(dataset.join("manifest.json")).unwrap()).unwrap();
    let listed = manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["path"] == "cpt/raw.jsonl")
        .expect("the manifest should list the raw sections");
    assert_eq!(listed["samples"], expected_ids.len());

    assert!(
        stdout.contains(&format!("folder: {}", dataset.display())),
        "{stdout}"
    );
    assert!(stdout.contains("dry run: no model was asked"), "{stdout}");
    assert!(stdout.contains("cost of this run: ≈ $0.00"), "{stdout}");
    assert!(
        !stand_in_work.join("calls").exists(),
        "a dry run starts no model"
    );
}
