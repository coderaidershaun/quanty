//! Runs the whole chapter conversion on the sample chapter with stubbed paid services, and the
//! built command with no services at all. Poppler and the file system are real.

// SMELL: this file is near 400 lines, where a file must be split. A new test of a chapter run
// needs a file of its own, and the checks that two files share then need a module of their own.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use ocr::ConversionSummary;
use ocr::content::RelationshipKind;
use ocr::convert::{PageProgress, convert_chapter_with_progress};
use ocr::reader::{PieceId, read_chapter};
use ocr::testing::{Call, Scenario, StubServices, page_folder, read_json, sample_job, sample_pdf};

use crate::figure_pictures::{
    assert_figure_pictures_read_back, assert_figures_cut, assert_hard_fallbacks,
};

const SOFT_HYPHEN: char = '\u{AD}';

fn strings(value: &serde_json::Value) -> Vec<&str> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item.as_str().unwrap())
        .collect()
}

fn call_steps(page: &serde_json::Value) -> Vec<&str> {
    page["conversion"]["calls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|call| call["step"].as_str().unwrap())
        .collect()
}

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

fn assert_routes_reasons_and_calls(chapter: &Path, stubs: &StubServices) {
    let expected: [(&str, &[&str], &[&str]); 7] = [
        ("haiku-copy", &[], &["tag", "copy"]),
        (
            "haiku-then-sonnet",
            &["copy-flagged-stronger-model"],
            &["tag", "copy", "transcribe"],
        ),
        (
            "haiku-then-sonnet",
            &["copy-contains-backslash"],
            &["tag", "copy", "transcribe"],
        ),
        (
            "haiku-then-sonnet",
            &["copy-failed-copy-check"],
            &["tag", "copy", "transcribe", "transcribe"],
        ),
        (
            "haiku-then-sonnet",
            &["copy-failed-copy-check"],
            &["tag", "copy", "transcribe", "transcribe"],
        ),
        ("sonnet", &["tags-report-table"], &["tag", "transcribe"]),
        ("sonnet", &["math-check-failed"], &["tag", "transcribe"]),
    ];
    for (position, (route, reasons, steps)) in (1..=7).zip(expected) {
        let page = read_json(&page_folder(chapter, position).join("page.json"));
        assert_eq!(page["format-version"], 1);
        assert_eq!(page["page-position"], position);
        let shown = match position {
            6 => "9".to_owned(),
            7 => "10".to_owned(),
            other => other.to_string(),
        };
        assert_eq!(
            page["printed-page-number"],
            shown.as_str(),
            "page {position}"
        );
        let conversion = &page["conversion"];
        assert_eq!(conversion["route"], route, "page {position}");
        assert_eq!(
            strings(&conversion["route-reasons"]),
            reasons,
            "page {position}"
        );
        assert_eq!(conversion["tags"].as_object().unwrap().len(), 14);
        for ratio in ["piece-words-in-text-layer", "text-layer-words-in-pieces"] {
            assert!(
                conversion["checks"]["word-match"][ratio].is_number(),
                "page {position} lacks {ratio}"
            );
        }
        assert_eq!(call_steps(&page), steps, "page {position}");
        for call in page["conversion"]["calls"].as_array().unwrap() {
            assert_eq!(call["input-tokens"], 100, "page {position}: {call}");
        }
        assert_eq!(
            stubs.calls_for(position).len(),
            steps.len() + 1,
            "page {position}: the math check is a call too"
        );
    }
    let page_seven = read_json(&page_folder(chapter, 7).join("page.json"));
    assert_eq!(page_seven["conversion"]["math-check"], "no-answer");
    let failure = page_seven["conversion"]["math-check-failure"]
        .as_str()
        .expect("page 7 should say why its math check gave no answer");
    assert!(
        failure.contains("status 503") && failure.contains("stub outage"),
        "{failure}"
    );
    let page_six = read_json(&page_folder(chapter, 6).join("page.json"));
    assert!(page_six["conversion"]["math-check-failure"].is_null());
    assert_eq!(
        stubs.calls_for(1),
        [Call::Tag(1), Call::Math(1), Call::Copy(1)]
    );
    assert_eq!(stubs.calls().len(), 27);
}

fn assert_relationships_saved(chapter: &Path) {
    let page_two = read_json(&page_folder(chapter, 2).join("page.json"));
    let relationships: Vec<(&str, u64, u64)> = page_two["relationships"]
        .as_array()
        .unwrap()
        .iter()
        .map(|link| {
            (
                link["kind"].as_str().unwrap(),
                link["from"].as_u64().unwrap(),
                link["to"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        relationships,
        [
            ("introduces", 2, 3),
            ("discusses", 4, 5),
            ("footnote-of", 7, 4)
        ]
    );
    let cites = page_two["pieces"][3]["cites"].as_array().unwrap();
    assert!(
        cites
            .iter()
            .any(|cite| cite["kind"] == "footnote" && cite["label"] == "1")
    );
}

fn assert_summary_counts_and_lists(summary: &ConversionSummary) {
    assert_eq!(
        (
            summary.calls.tag,
            summary.calls.math_check,
            summary.calls.copy,
            summary.calls.transcribe
        ),
        (7, 7, 5, 8)
    );
    assert_eq!(
        (
            summary.routes.haiku_copy,
            summary.routes.sonnet,
            summary.routes.haiku_then_sonnet
        ),
        (1, 2, 4)
    );
    let printed = summary.to_string();
    assert!(
        printed.contains("printed page numbers out of sequence: page 6 shows 9\n"),
        "{printed}"
    );
    assert!(
        printed
            .contains("tokens of stub-transcriber: 800 in, 80 out, 0 cache read, 0 cache write\n"),
        "{printed}"
    );
    let to_check = printed
        .lines()
        .find(|line| line.starts_with("pages to check:"))
        .unwrap();
    assert!(!to_check.contains("page 1 "), "{to_check}");
    for position in 2..=7 {
        assert!(
            to_check.contains(&format!("page {position} (low word match")),
            "{to_check}"
        );
    }
    for expected in [
        "page 2 (low word match, figure image was not checked against the page's text)",
        "page 3 (low word match, figure image is most of the page, figure image holds body text, figure image was not checked against the page's text)",
        "page 4 (low word match, figure image is the whole page)",
        "page 5 (low word match, figure image is the whole page)",
    ] {
        assert!(
            to_check.contains(expected),
            "{expected} missing from {to_check}"
        );
    }
}

/// The lead-in of the formula that opens page 7 is found on page 6, though no saved
/// relationship crosses a page.
fn assert_reads_back_with_bridged_lead_in(chapter: &Path, summary: &ConversionSummary) {
    let read_back = read_chapter(chapter).unwrap();
    let saved_pieces = summary.pieces;
    assert_eq!(
        read_back.pieces.len() as u32,
        saved_pieces.heading
            + saved_pieces.text
            + saved_pieces.formula
            + saved_pieces.figure
            + saved_pieces.table
            + saved_pieces.footnote
    );
    let page_seven = read_json(&page_folder(chapter, 7).join("page.json"));
    assert!(page_seven["relationships"].as_array().unwrap().is_empty());
    let lead_in = PieceId { page: 6, number: 4 };
    let formula = PieceId { page: 7, number: 1 };
    for end in [lead_in, formula] {
        let piece = read_back.piece(end).unwrap();
        assert!(
            piece
                .relationships
                .iter()
                .any(|link| link.kind == RelationshipKind::Introduces
                    && link.from == lead_in
                    && link.to == formula),
            "{end:?} lacks the bridged edge"
        );
    }
}

/// Pages end in any order, so the positions are sorted before they are compared.
fn assert_each_page_told_once_with_its_cost(heard: &[PageProgress], summary: &ConversionSummary) {
    assert_eq!(
        heard.first(),
        Some(&PageProgress::Pages {
            total: 7,
            done_before: 0
        })
    );
    let mut positions = Vec::new();
    let mut cost_usd = 0.0;
    // Input tokens, output tokens and cents, for each model.
    let mut told: BTreeMap<&str, [u64; 3]> = BTreeMap::new();
    let cents = |usd: f64| (usd * 100.0).round() as u64;
    for progress in &heard[1..] {
        match progress {
            PageProgress::PageDone { position, calls } => {
                positions.push(*position);
                for (model, used) in &calls.by_model {
                    cost_usd += used.cost_usd;
                    let sum = told.entry(model.as_str()).or_default();
                    sum[0] += used.input_tokens;
                    sum[1] += used.output_tokens;
                    sum[2] += cents(used.cost_usd);
                }
            }
            other => panic!("only finished pages should follow the count, got {other:?}"),
        }
    }
    positions.sort_unstable();
    assert_eq!(positions, (1..=7).collect::<Vec<u32>>());
    let in_summary: BTreeMap<&str, [u64; 3]> = (summary.calls.by_model.iter())
        .map(|(model, used)| {
            let figures = [used.input_tokens, used.output_tokens, cents(used.cost_usd)];
            (model.as_str(), figures)
        })
        .collect();
    let expected = [
        ("stub-copier", [500, 50, 5]),
        ("stub-tagger", [700, 70, 7]),
        ("stub-transcriber", [800, 80, 8]),
    ];
    assert_eq!(in_summary, BTreeMap::from(expected));
    assert_eq!(
        told, in_summary,
        "the pages tell the tokens and the cost of the run"
    );
    let reported = summary.calls.by_model.values().map(|used| used.cost_usd);
    let summary_cost: f64 = reported.sum();
    assert!(
        (cost_usd - summary_cost).abs() < 1e-9,
        "the pages told {cost_usd}, the summary says {summary_cost}"
    );
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
