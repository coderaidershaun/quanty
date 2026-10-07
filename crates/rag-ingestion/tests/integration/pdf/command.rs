//! Runs the built `rag-ingest pdf` command as a person would: first the refusals, which need no
//! store and no key, and then the one run that gets as far as the conversion.

use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Output};

use graph::testing::stored_document;
use ocr::testing::{sample_job, sample_pdf};
use rag_core::DocId;

use crate::support::{RunRagIngest, ThrowawayStores};

fn stderr_text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Runs `rag-ingest pdf` in `working_folder`, which holds an empty `.env` file, so the real
/// settings stay out. Both stores are at closed ports and no key is set, so a refusal that came
/// after a store or a key was needed would show another message.
fn run_pdf_refused(working_folder: &Path, arguments: &[&OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rag-ingest"))
        .arg("pdf")
        .args(arguments)
        .current_dir(working_folder)
        .env("QDRANT_URL", "http://127.0.0.1:1")
        .env("FALKORDB_URL", "falkor://127.0.0.1:1")
        .env("CONTENT_DIR", working_folder.join("content"))
        .env("CONCEPT_CACHE_DIR", working_folder.join("cache"))
        .env(
            "CONCEPT_DECISION_LOG",
            working_folder.join("concept-decisions.jsonl"),
        )
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("CONVERTER_JEV_API_KEY")
        .env_remove("EMBEDDING_GEMINI_API_KEY")
        .output()
        .expect("the rag-ingest binary should start")
}

#[test]
fn pdf_refuses_a_missing_book_a_bad_file_name_and_a_missing_file_before_anything_starts() {
    let working_folder = tempfile::tempdir().unwrap();
    std::fs::write(working_folder.path().join(".env"), "").unwrap();
    let named_right = working_folder.path().join("chapter-3-a-name.pdf");
    let named_wrong = working_folder.path().join("notes.pdf");
    let not_there = working_folder.path().join("chapter-3-not-there.pdf");
    std::fs::write(&named_right, "").unwrap();
    std::fs::write(&named_wrong, "").unwrap();

    let no_book = run_pdf_refused(working_folder.path(), &[named_right.as_ref()]);
    let bad_name = run_pdf_refused(
        working_folder.path(),
        &["--book".as_ref(), "A Book".as_ref(), named_wrong.as_ref()],
    );
    let missing_file = run_pdf_refused(
        working_folder.path(),
        &["--book".as_ref(), "A Book".as_ref(), not_there.as_ref()],
    );

    for (output, code, expected) in [
        (no_book, 2, vec!["--book"]),
        (
            bad_name,
            1,
            vec!["chapter-<number>-<name>.pdf", "notes.pdf"],
        ),
        (missing_file, 1, vec!["is not there"]),
    ] {
        let message = stderr_text(&output);
        assert_eq!(output.status.code(), Some(code), "{message}");
        for needle in expected {
            assert!(
                message.contains(needle),
                "{needle} is missing from: {message}"
            );
        }
        assert!(
            output.stdout.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(
            !working_folder.path().join("content").exists(),
            "a refusal must not make the content folder"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored pdf::"]
async fn pdf_without_the_converter_key_stops_before_any_page_is_converted() {
    let throwaway = ThrowawayStores::new("pdf-no-key");
    let config = throwaway.config();
    let pdf = sample_pdf();

    let output = throwaway.rag_ingest_pdf([
        OsStr::new("--book"),
        OsStr::new("Option Volatility and Pricing"),
        pdf.as_os_str(),
    ]);

    let message = stderr_text(&output);
    assert_eq!(output.status.code(), Some(1), "{message}");
    assert!(message.contains("CONVERTER_JEV_API_KEY"), "{message}");
    assert!(output.stdout.is_empty());
    assert!(
        !config.content_folder.exists(),
        "no page was converted, so nothing was saved"
    );
    let document =
        DocId::from_source_sha256(&sample_job(&config.content_folder).source_sha256().unwrap());
    let stores = throwaway.connect().await;
    assert!(
        stored_document(&stores.graph, document).await.is_none(),
        "the graph has no such document"
    );
}
