//! Runs the whole chapter conversion on the sample chapter with stubbed paid services, and the
//! built command with no services at all. Poppler and the file system are real.

use std::path::Path;
use std::process::Command;

use ocr::convert::convert_chapter_with_progress;
use ocr::testing::{Scenario, StubServices, page_folder, read_json, sample_job, sample_pdf};

use crate::figure_pictures::{
    assert_figure_pictures_read_back, assert_figures_cut, assert_hard_fallbacks,
};
use crate::relationships::{assert_reads_back_with_bridged_lead_in, assert_relationships_saved};
use crate::routes::assert_routes_reasons_and_calls;
use crate::summary::{assert_each_page_told_once_with_its_cost, assert_summary_counts_and_lists};

const SOFT_HYPHEN: char = '\u{AD}';

/// Runs the built command in an empty folder, so no `.env` file is found.
fn run_command(arguments: &[&std::ffi::OsStr], api_key: Option<&str>) -> std::process::Output {
    let working_folder = tempfile::tempdir().unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_ocr"));
    command
        .args(arguments)
        .current_dir(working_folder.path())
        .env_remove("CONVERTER_JEV_API_KEY")
        .env_remove("ANTHROPIC_API_KEY");
    if let Some(key) = api_key {
        command.env("ANTHROPIC_API_KEY", key);
    }
    command.output().unwrap()
}

/// The text layer holds soft hyphens by design, so only the saved index and pieces are searched.
fn assert_pages_saved_without_soft_hyphens(chapter: &Path) {
    for position in 1..=7 {
        let folder = page_folder(chapter, position);
        for file in ["page.pdf", "page.png", "text-layer.txt", "page.json"] {
            assert!(folder.join(file).is_file(), "page {position} lacks {file}");
        }
        assert!(
            !chapter
                .join(format!("page-num-{position}.partial"))
                .exists()
        );
        let page_json = std::fs::read_to_string(folder.join("page.json")).unwrap();
        assert!(!page_json.contains(SOFT_HYPHEN), "page {position}");
        let page = read_json(&folder.join("page.json"));
        for (index, piece) in page["pieces"].as_array().unwrap().iter().enumerate() {
            assert_eq!(piece["number"], index + 1);
            let file = folder.join(piece["file"].as_str().unwrap());
            assert!(
                !std::fs::read_to_string(&file)
                    .unwrap()
                    .contains(SOFT_HYPHEN),
                "an invisible soft hyphen was saved in {}",
                file.display()
            );
        }
    }
    // Page 1 is a Haiku copy; the cleaning pass removed its soft hyphen.
    let first_text = std::fs::read_to_string(page_folder(chapter, 1).join("01-text.md")).unwrap();
    assert!(first_text.contains("settlement period"));
}

#[tokio::test]
async fn chapter_converts_then_reruns_without_calls() {
    let root = tempfile::tempdir().unwrap();
    let job = sample_job(root.path());
    let stubs = StubServices::new(Scenario::SampleChapter);
    let mut heard = Vec::new();

    let summary = convert_chapter_with_progress(&job, &stubs, |progress| heard.push(progress))
        .await
        .unwrap();

    let chapter = job.chapter_folder();
    assert_eq!(read_json(&chapter.join("chapter.json"))["finished"], true);
    assert_pages_saved_without_soft_hyphens(&chapter);
    assert_routes_reasons_and_calls(&chapter, &stubs);
    assert_relationships_saved(&chapter);
    assert_figures_cut(&chapter);
    assert_hard_fallbacks(&chapter, &stubs);
    assert_summary_counts_and_lists(&summary);
    assert_reads_back_with_bridged_lead_in(&chapter, &summary);
    assert_figure_pictures_read_back(&chapter);
    assert_each_page_told_once_with_its_cost(&heard, &summary);

    let calls_before = stubs.calls().len();
    let mut heard_again = Vec::new();
    let again = convert_chapter_with_progress(&job, &stubs, |progress| heard_again.push(progress))
        .await
        .unwrap();
    assert_eq!(stubs.calls().len(), calls_before);
    assert!(heard_again.is_empty(), "{heard_again:?}");
    assert_eq!(
        (again.routes, again.pieces),
        (summary.routes, summary.pieces)
    );
    assert_eq!(
        again.calls.tag + again.calls.math_check + again.calls.copy + again.calls.transcribe,
        0
    );

    let output = run_command(
        &[
            "--book".as_ref(),
            "Option Volatility and Pricing".as_ref(),
            "--out".as_ref(),
            root.path().as_os_str(),
            sample_pdf().as_os_str(),
        ],
        None,
    );
    let printed = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{printed} {}",
        String::from_utf8_lossy(&output.stderr)
    );
    for expected in ["pages: 7", "routes:", "pieces:", "calls this run: 0"] {
        assert!(
            printed.contains(expected),
            "{expected} missing from {printed}"
        );
    }
}
