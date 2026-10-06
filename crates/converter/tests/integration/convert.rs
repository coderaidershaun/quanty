//! Runs the whole chapter conversion on the sample chapter with stubbed paid services, and the
//! built command with no services at all. Poppler and the file system are real.

use std::error::Error;
use std::path::Path;
use std::process::Command;

use converter::ConversionSummary;
use converter::content::{ContentError, RelationshipKind};
use converter::convert::{ConvertError, convert_chapter_with};
use converter::reader::{PieceId, read_chapter};

use crate::figure_pictures::{
    assert_figure_pictures_read_back, assert_figures_cut, assert_hard_fallbacks,
};
use crate::stubs::{Call, Scenario, StubServices, page_folder, read_json, sample_job, sample_pdf};

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
    let mut command = Command::new(env!("CARGO_BIN_EXE_converter"));
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

/// Every page folder is whole, its pieces are numbered in order, and no saved index or piece
/// file holds a soft hyphen. The text layer holds them by design, so it is left out of the search.
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
    // Page 1 is a Haiku copy, and the cleaning pass took the soft hyphen out of it.
    let first_text = std::fs::read_to_string(page_folder(chapter, 1).join("01-text.md")).unwrap();
    assert!(first_text.contains("settlement period"));
}

/// Each page took the branch its scenario was built for, and its `page.json` says so.
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
        assert_eq!(
            stubs.calls_for(position).len(),
            steps.len() + 1,
            "page {position}: the math check is a call too"
        );
    }
    assert_eq!(
        read_json(&page_folder(chapter, 7).join("page.json"))["conversion"]["math-check"],
        "no-answer"
    );
    assert_eq!(
        stubs.calls_for(1),
        [Call::Tag(1), Call::Math(1), Call::Copy(1)]
    );
    assert_eq!(stubs.calls().len(), 27);
}

/// A page with the standard transcription keeps all three kinds of relationship, and the
/// footnote citation the converter adds from the marker.
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

/// The writer and the reader agree, and the lead-in of the formula that opens page 7 is found on
/// page 6 although no saved relationship crosses a page.
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

#[tokio::test]
async fn chapter_converts_then_reruns_without_calls() {
    let root = tempfile::tempdir().unwrap();
    let job = sample_job(root.path());
    let stubs = StubServices::new(Scenario::SampleChapter);

    let summary = convert_chapter_with(&job, &stubs).await.unwrap();

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

    // A second run changes nothing and makes no call.
    let calls_before = stubs.calls().len();
    let again = convert_chapter_with(&job, &stubs).await.unwrap();
    assert_eq!(stubs.calls().len(), calls_before);
    assert_eq!(
        (again.routes, again.pieces),
        (summary.routes, summary.pieces)
    );
    assert_eq!(
        again.calls.tag + again.calls.math_check + again.calls.copy + again.calls.transcribe,
        0
    );

    // The built command does the same with no key of any kind.
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

#[tokio::test]
async fn failing_page_is_named_and_the_next_run_resumes() {
    let root = tempfile::tempdir().unwrap();
    let job = sample_job(root.path());
    let failing = StubServices::new(Scenario::AllTables {
        broken_page: Some(7),
    });

    let error = convert_chapter_with(&job, &failing).await.unwrap_err();

    let chapter = job.chapter_folder();
    let rejected = chapter.join("page-num-7.partial/rejected-reply.json");
    assert!(
        matches!(
            &error,
            ConvertError::PageFailed { position: 7, source, .. }
                if matches!(**source, converter::PageError::ReplyRejected { .. })
        ),
        "{error:?}"
    );
    let mut chain = vec![error.to_string()];
    let mut source = error.source();
    while let Some(next) = source {
        chain.push(next.to_string());
        source = next.source();
    }
    let chain = chain.join(": ");
    assert!(chain.contains("page 7"), "{chain}");
    assert!(chain.contains("page-num-7.partial"), "{chain}");
    assert!(chain.contains(&rejected.display().to_string()), "{chain}");

    let transcriptions: Vec<Call> = failing
        .calls_for(7)
        .into_iter()
        .filter(|call| matches!(call, Call::Transcribe { .. }))
        .collect();
    assert_eq!(transcriptions.len(), 2);
    assert!(matches!(
        &transcriptions[0],
        Call::Transcribe {
            correction: None,
            ..
        }
    ));
    assert!(matches!(
        &transcriptions[1],
        Call::Transcribe {
            correction: Some(_),
            ..
        }
    ));
    assert!(
        std::fs::read_to_string(&rejected)
            .unwrap()
            .contains("frac{a")
    );
    for position in 1..=6 {
        assert!(page_folder(&chapter, position).join("page.json").is_file());
    }
    assert!(!page_folder(&chapter, 7).exists());
    assert_eq!(read_json(&chapter.join("chapter.json"))["finished"], false);

    // A page.json that cannot be read for any reason except being missing or malformed is an
    // error to report, not a reason to delete the page and pay to convert it again.
    let page_json = page_folder(&chapter, 3).join("page.json");
    let saved_page_json = std::fs::read(&page_json).unwrap();
    std::fs::remove_file(&page_json).unwrap();
    std::fs::create_dir(&page_json).unwrap();
    let unreadable = StubServices::new(Scenario::AllTables { broken_page: None });
    let error = convert_chapter_with(&job, &unreadable).await.unwrap_err();
    assert!(
        matches!(error, ConvertError::Content(ContentError::Read { .. })),
        "{error:?}"
    );
    assert!(unreadable.calls().is_empty());
    assert!(page_folder(&chapter, 3).join("02-text.md").is_file());
    std::fs::remove_dir(&page_json).unwrap();
    std::fs::write(&page_json, saved_page_json).unwrap();

    let working = StubServices::new(Scenario::AllTables { broken_page: None });
    let summary = convert_chapter_with(&job, &working).await.unwrap();

    assert!(working.calls().iter().all(|call| call.position() == 7));
    assert!(!working.calls().is_empty());
    assert!(!chapter.join("page-num-7.partial").exists());
    assert_eq!(read_json(&chapter.join("chapter.json"))["finished"], true);
    assert_eq!((summary.converted_now, summary.already_done), (1, 6));
}

#[tokio::test]
async fn different_pdf_for_the_same_chapter_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let job = sample_job(root.path());
    let stubs = StubServices::new(Scenario::SampleChapter);
    convert_chapter_with(&job, &stubs).await.unwrap();
    let chapter_json = job.chapter_folder().join("chapter.json");
    let before = std::fs::read(&chapter_json).unwrap();

    let other_folder = tempfile::tempdir().unwrap();
    let other_pdf = other_folder.path().join("chapter-1-sample-pages.pdf");
    let one_page = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pdfs/text.pdf");
    std::fs::copy(one_page, &other_pdf).unwrap();
    let other_job =
        converter::ChapterJob::new("Option Volatility and Pricing", &other_pdf, root.path())
            .unwrap();

    let error = convert_chapter_with(&other_job, &stubs).await.unwrap_err();

    match error {
        ConvertError::DifferentSource { folder, .. } => assert_eq!(folder, job.chapter_folder()),
        other => panic!("expected DifferentSource, got {other:?}"),
    }
    assert_eq!(std::fs::read(&chapter_json).unwrap(), before);
}

#[test]
fn cli_refuses_missing_book_bad_file_name_and_api_key() {
    let no_book = run_command(&["chapter-1-x.pdf".as_ref()], None);
    let bad_name = run_command(
        &["--book".as_ref(), "A Book".as_ref(), "notes.pdf".as_ref()],
        None,
    );
    let key_set = run_command(
        &[
            "--book".as_ref(),
            "A Book".as_ref(),
            "chapter-1-x.pdf".as_ref(),
        ],
        Some("set-for-this-test"),
    );

    for (output, expected) in [
        (no_book, vec!["missing --book"]),
        (bad_name, vec!["chapter-<number>-<name>.pdf", "notes.pdf"]),
        (key_set, vec!["ANTHROPIC_API_KEY"]),
    ] {
        let message = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{message}");
        for needle in expected {
            assert!(message.contains(needle), "{needle} missing from {message}");
        }
    }
}
