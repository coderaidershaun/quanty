//! Which documents a build trains on, which it holds out, and which parts of a book it leaves
//! out, through `build` as another crate calls it.

use std::fs;
use std::path::{Path, PathBuf};

use ocr::ChapterIndex;
use rag_core::DocId;
use rag_dataset::{BuildError, BuildOptions, Report, SplitCounts, build};
use rag_ingestion::testing::StandInLlm;
use serde_json::{Value, json};

use crate::support;

/// The three sample chapters, and a copy of the second chapter of the notes as the index of that
/// book, in a content folder of their own. The copy has a sentence more in its first paragraph,
/// because a section whose text is the same as that of a kept section is left out as a copy.
fn content_with_an_index(parent: &Path) -> PathBuf {
    let content = parent.join("content");
    for chapter in support::sample_chapters() {
        let relative = chapter.strip_prefix(support::sample_content()).unwrap();
        support::copy_folder(&chapter, &content.join(relative));
    }
    let index = content.join("quanty-sample-notes/index");
    support::copy_folder(&support::in_depth_chapter(), &index);
    let first_paragraph = index.join("page-num-1/02-text.md");
    let text = fs::read_to_string(&first_paragraph).unwrap();
    fs::write(
        first_paragraph,
        format!(
            "{}\nThis index points into the chapters of the book.\n",
            text.trim_end()
        ),
    )
    .unwrap();
    let file = index.join("chapter.json");
    let mut saved: Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
    saved["name"] = json!({ "title": "Quanty Sample Notes, Index" });
    saved["source-sha256"] =
        json!("c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3");
    fs::write(file, serde_json::to_string_pretty(&saved).unwrap()).unwrap();
    content
}

/// Runs the build on a task of its own, as the servers of this project do, so that it must be
/// `Send`.
async fn spawned_build(options: BuildOptions) -> Result<Report, BuildError> {
    tokio::spawn(async move {
        let llm = StandInLlm::finding_nothing();
        build(&options, &llm, None, &mut |_, _| {}).await
    })
    .await
    .expect("the build task should not panic")
}

fn documents_in(file: &Path) -> Vec<String> {
    support::read_jsonl(file)
        .iter()
        .map(|record| record["source"]["document"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn held_out_documents_and_book_parts_follow_the_selection() {
    let work = tempfile::tempdir().unwrap();
    let content = content_with_an_index(work.path());
    let out = work.path().join("one");
    let notes_one = ChapterIndex::read(&support::intuition_chapter()).unwrap();
    let mut options = support::options_over(&content, &out);
    options.generation = false;
    options.selection.holdout = vec![
        DocId::from_source_sha256(&notes_one.source_sha256).to_string(),
        "OPTION VOLATILITY AND PRICING, CHAPTER 1: SAMPLE PAGES".to_owned(),
    ];

    let report = spawned_build(options.clone()).await.unwrap();

    let heldout = support::read_jsonl(&out.join("cpt/heldout.jsonl"));
    assert!(heldout.iter().all(|record| record["split"] == "heldout"));
    assert_eq!(
        documents_in(&out.join("cpt/heldout.jsonl")),
        [
            "Option Volatility and Pricing, chapter 1: Sample Pages",
            "Quanty Sample Notes, chapter 1: Options Pricing Intuition"
        ]
    );
    assert_eq!(
        documents_in(&out.join("cpt/raw.jsonl")),
        ["Quanty Sample Notes, chapter 2: Black Scholes In Depth"],
        "the index of the book is left out"
    );
    assert_eq!(
        report.documents,
        SplitCounts {
            train: 1,
            heldout: 2
        }
    );

    options.include_book_parts = true;
    spawned_build(options.clone()).await.unwrap();

    let raw = support::read_jsonl(&out.join("cpt/raw.jsonl"));
    let index = raw
        .iter()
        .find(|record| record["source"]["document"] == "Quanty Sample Notes, Index")
        .expect("the index is built when the parts of a book are asked for");
    assert_eq!(index["source"]["doc_role"], "part");
    assert_eq!(index["split"], "train");
    assert_eq!(documents_in(&out.join("cpt/heldout.jsonl")).len(), 2);

    let missing_out = work.path().join("two");
    options.out = missing_out.clone();
    options.selection.holdout = vec!["No Such Document".to_owned()];
    let result = spawned_build(options).await;
    assert!(
        matches!(result, Err(BuildError::Selection(_))),
        "a misspelt held-out document must stop the build, or it would be trained on: {result:?}"
    );
    assert!(!missing_out.join("cpt/raw.jsonl").exists());
}
