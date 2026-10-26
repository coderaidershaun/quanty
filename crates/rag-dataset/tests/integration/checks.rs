//! The checks of a build, through `build`: the page gate, the formula and Python checks, the
//! copies, and the shaping of text (a word cut at a hyphen, a separate accent), over a chapter
//! written here and over the sample chapters.

use std::fs;
use std::path::{Path, PathBuf};

use rag_dataset::{BuildOptions, Report, build};
use rag_ingestion::testing::StandInLlm;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::support;

/// A paragraph of this many tokens, led by a word that tells it from the others.
fn paragraph(name: &str, tokens: usize) -> String {
    format!("{name} {}", "abc ".repeat(tokens - 1).trim_end())
}

/// A piece of a page: its kind, the extension of its file, its text and the fields its entry adds.
struct Piece {
    kind: &'static str,
    extension: &'static str,
    text: String,
    extra: Value,
}

fn heading(text: &str) -> Piece {
    Piece {
        kind: "heading",
        extension: "md",
        text: text.to_owned(),
        extra: json!({ "rank": 1 }),
    }
}

fn text(text: String) -> Piece {
    Piece {
        kind: "text",
        extension: "md",
        text,
        extra: json!({}),
    }
}

fn formula(latex: &str) -> Piece {
    Piece {
        kind: "formula",
        extension: "tex",
        text: latex.to_owned(),
        extra: json!({
            "label": "(1.1)",
            "statement": "A fraction with no closing brace."
        }),
    }
}

fn python_listing(code: &str) -> Piece {
    Piece {
        kind: "code",
        extension: "txt",
        text: code.to_owned(),
        extra: json!({
            "language": "python",
            "summary": "A function that does not compile."
        }),
    }
}

fn write_json(file: &Path, value: &Value) {
    fs::write(file, serde_json::to_string_pretty(value).unwrap()).unwrap();
}

/// Writes one page of a chapter. `page` holds what `page.json` keeps besides its pieces: the
/// mid-sentence flags and the conversion.
// SMELL: another test file writes hand-made pages with its own copy of this code, so a change to
// the saved page format must be made in both.
fn write_page(chapter: &Path, position: u32, mut page: Value, pieces: Vec<Piece>) {
    let folder = chapter.join(format!("page-num-{position}"));
    fs::create_dir_all(&folder).unwrap();
    let mut entries = Vec::new();
    for (index, piece) in pieces.into_iter().enumerate() {
        let number = index + 1;
        let file = format!("{number:02}-{}.{}", piece.kind, piece.extension);
        fs::write(folder.join(&file), format!("{}\n", piece.text)).unwrap();
        let mut entry = json!({ "number": number, "file": file, "kind": piece.kind });
        entry
            .as_object_mut()
            .unwrap()
            .extend(piece.extra.as_object().unwrap().clone());
        entries.push(entry);
    }
    let saved = page.as_object_mut().unwrap();
    saved.insert("format-version".into(), json!(1));
    saved.insert("page-position".into(), json!(position));
    saved.insert("pieces".into(), Value::Array(entries));
    saved.insert("relationships".into(), json!([]));
    write_json(&folder.join("page.json"), &page);
}

/// The conversion record of the first page of the real sample chapter, whole, with the share of
/// the page's words that the text layer also holds set to `word_match`.
fn conversion_matching(word_match: f64) -> Value {
    let page = fs::read_to_string(support::sample_chapter().join("page-num-1/page.json")).unwrap();
    let mut page: Value = serde_json::from_str(&page).unwrap();
    let mut conversion = page["conversion"].take();
    conversion["route-reasons"] = json!([]);
    conversion["checks"]["word-match"]["piece-words-in-text-layer"] = json!(word_match);
    conversion
}

/// Writes the chapter "Checked Chapter" of "Check Notes" in `<content>/check-notes/chapter-1`.
/// Every heading is of rank 1, so each is a section of its own, and each section is over 500
/// tokens, so none is joined to another; the chapter is over 8,000 tokens, so it is cut along its
/// headings.
///
/// - page 1, converted, with 0.8 of its words in the text layer: "Low Page".
/// - page 2, by hand: "Bad Formula" with a formula that is missing a brace, "Bad Listing" with a
///   Python function that does not compile and holds a dollar sign, "Copy" twice with the same
///   text, and "Cut Word at the Café", whose é is written as an e and a separate accent, and whose
///   last paragraph stops at "deriva-".
/// - page 3, by hand: the rest of that paragraph, "tive of the price", and a second é written
///   with a separate accent.
fn write_checked_chapter(content: &Path) -> PathBuf {
    let chapter = content.join("check-notes/chapter-1");
    fs::create_dir_all(&chapter).unwrap();
    write_json(
        &chapter.join("chapter.json"),
        &json!({
            "format-version": 1,
            "media-title": "Check Notes",
            "name": { "chapter": { "number": 1, "name": "Checked Chapter" } },
            "source-file": "chapter-1-checked-chapter.pdf",
            "source-sha256": "2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f",
            "page-count": 3,
            "finished": true,
        }),
    );
    write_page(
        &chapter,
        1,
        json!({ "conversion": conversion_matching(0.8) }),
        vec![heading("Low Page"), text(paragraph("LOW", 1500))],
    );
    write_page(
        &chapter,
        2,
        json!({ "ends-mid-sentence": true }),
        vec![
            heading("Bad Formula"),
            text(paragraph("FORMULA", 1500)),
            formula("\\frac{a}{b"),
            heading("Bad Listing"),
            text(paragraph("LISTING", 1500)),
            python_listing("def broken(:\n    return \"$\""),
            heading("Copy"),
            text(paragraph("COPY", 1500)),
            heading("Copy"),
            text(paragraph("COPY", 1500)),
            heading("Cut Word at the Cafe\u{301}"),
            text(format!("{} the deriva-", paragraph("CUT", 1500))),
        ],
    );
    write_page(
        &chapter,
        3,
        json!({ "starts-mid-sentence": true }),
        vec![text(
            "tive of the price, as every cafe\u{301} owner knows.".to_owned(),
        )],
    );
    chapter
}

/// The settings of a dry run of the content folder into `out`, with every other default.
fn dry_run_over(content: &Path, out: &Path) -> BuildOptions {
    let mut options = support::options_over(content, out);
    options.generation = false;
    options
}

/// Builds with a stand-in for the model and no graph.
async fn build_with(options: &BuildOptions) -> Report {
    build(
        options,
        &StandInLlm::finding_nothing(),
        None,
        &mut |_, _| {},
    )
    .await
    .expect("the build should finish")
}

/// The record whose text holds this word.
fn record_with<'a>(records: &'a [Value], word: &str) -> &'a Value {
    let found: Vec<&Value> = records
        .iter()
        .filter(|record| record["text"].as_str().unwrap().contains(word))
        .collect();
    assert_eq!(found.len(), 1, "one record holds {word:?}");
    found[0]
}

/// The text of a record after its heading line and the blank line that follows it.
fn text_after_heading_line(record: &Value) -> &str {
    record["text"]
        .as_str()
        .unwrap()
        .split_once("\n\n")
        .unwrap()
        .1
}

fn sha256_hex(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Writes the chapter "Other Chapter" of "Check Notes" in `<content>/check-notes/chapter-2`: one
/// page by hand with the same "Copy" section as `write_checked_chapter`. Its section sits under
/// another document title, so its heading line differs and the text after that line does not.
fn write_copy_chapter(content: &Path) {
    let chapter = content.join("check-notes/chapter-2");
    fs::create_dir_all(&chapter).unwrap();
    write_json(
        &chapter.join("chapter.json"),
        &json!({
            "format-version": 1,
            "media-title": "Check Notes",
            "name": { "chapter": { "number": 2, "name": "Other Chapter" } },
            "source-file": "chapter-2-other-chapter.pdf",
            "source-sha256": "3a".repeat(32),
            "page-count": 1,
            "finished": true,
        }),
    );
    write_page(
        &chapter,
        1,
        json!({}),
        vec![heading("Copy"), text(paragraph("COPY", 1500))],
    );
}

#[tokio::test]
async fn a_dry_run_checks_every_section_of_a_chapter_written_by_hand() {
    let work = tempfile::tempdir().unwrap();
    let content = work.path().join("content");
    write_checked_chapter(&content);
    write_copy_chapter(&content);
    let out = work.path().join("out");

    let report = build_with(&dry_run_over(&content, &out)).await;

    let raw = support::read_jsonl(&out.join("cpt/raw.jsonl"));
    let rejected = support::read_jsonl(&out.join("cpt/rejected.jsonl"));
    a_section_on_a_page_that_matches_too_little_of_its_text_layer_is_rejected(
        &report, &raw, &rejected,
    );
    a_python_listing_that_does_not_compile_is_recorded_and_kept(&raw);
    the_second_copy_of_a_section_is_rejected_as_a_duplicate_of_the_first(
        &report, &raw, &rejected, &out,
    );
    a_word_that_a_page_break_cut_at_a_hyphen_is_joined_again(&raw);
    a_letter_written_with_a_separate_accent_comes_out_as_one_character(&raw);

    let bad = record_with(&rejected, "FORMULA abc");
    assert_eq!(bad["rejected"], "latex");
    assert_eq!(bad["quality"]["latex_renders"], false);
    assert_eq!(report.rejected["latex"], 1);
    let lenient_out = work.path().join("lenient");
    let mut lenient_options = dry_run_over(&content, &lenient_out);
    lenient_options.keep_unrendered = true;

    let lenient = build_with(&lenient_options).await;

    let raw = support::read_jsonl(&lenient_out.join("cpt/raw.jsonl"));
    assert_eq!(
        record_with(&raw, "FORMULA abc")["quality"]["latex_renders"],
        false,
        "the section is kept, and says that its formula does not render"
    );
    assert!(!lenient.rejected.contains_key("latex"));
    assert_eq!(
        record_with(&raw, "LISTING abc")["quality"]["latex_renders"],
        Value::Null,
        "a dollar sign in a listing is not math, and a section with no math says nothing"
    );
}

fn a_section_on_a_page_that_matches_too_little_of_its_text_layer_is_rejected(
    report: &Report,
    raw: &[Value],
    rejected: &[Value],
) {
    let low = record_with(rejected, "LOW abc");
    assert_eq!(low["rejected"], "page_score");
    assert_eq!(low["quality"]["page_score"], 0.8);
    assert_eq!(report.rejected["page_score"], 1);
    assert!(
        raw.iter()
            .all(|record| !record["text"].as_str().unwrap().contains("LOW abc")),
        "a rejected section is not in the dataset"
    );
    assert_eq!(
        record_with(raw, "LISTING abc")["quality"]["page_score"],
        Value::Null,
        "a page written by hand has no score"
    );
}

fn a_python_listing_that_does_not_compile_is_recorded_and_kept(raw: &[Value]) {
    assert_eq!(
        record_with(raw, "LISTING abc")["quality"]["code_parses"],
        false,
        "a listing that does not compile is no reason to keep the section out"
    );
    assert_eq!(
        record_with(raw, "CUT abc")["quality"]["code_parses"],
        Value::Null,
        "a section with no Python listing says nothing"
    );
}

fn the_second_copy_of_a_section_is_rejected_as_a_duplicate_of_the_first(
    report: &Report,
    raw: &[Value],
    rejected: &[Value],
    out: &Path,
) {
    let first = record_with(raw, "COPY abc");
    let first_section_id = first["id"].as_str().unwrap().strip_suffix(":raw").unwrap();
    let copies: Vec<&Value> = rejected
        .iter()
        .filter(|record| record["text"].as_str().unwrap().contains("COPY abc"))
        .collect();
    assert_eq!(
        copies.len(),
        2,
        "the second copy and the copy under another title"
    );
    for copy in &copies {
        assert_eq!(copy["rejected"], format!("duplicate_of {first_section_id}"));
    }
    assert!(
        copies
            .iter()
            .any(|copy| copy["source"]["document"] == "Check Notes, chapter 2: Other Chapter"),
        "one of the copies is in the other document"
    );
    assert_eq!(report.rejected["duplicate_of"], 2);

    let hash = sha256_hex(text_after_heading_line(first));
    assert_eq!(first["dedup"]["sha256"], hash);
    for copy in &copies {
        assert_eq!(copy["dedup"]["sha256"], hash, "a copy carries the hash too");
        assert_eq!(
            text_after_heading_line(copy),
            text_after_heading_line(first)
        );
    }
    assert_eq!(first["dedup"]["minhash_cluster"], Value::Null);

    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(out.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["rejected"]["sections"]["duplicate_of"], 2);
}

fn a_word_that_a_page_break_cut_at_a_hyphen_is_joined_again(raw: &[Value]) {
    let text = record_with(raw, "CUT abc")["text"].as_str().unwrap();
    assert!(
        text.contains("abc the derivative of the price"),
        "the two halves of the word are joined with the hyphen gone"
    );
    assert!(!text.contains("deriva-"));
}

fn a_letter_written_with_a_separate_accent_comes_out_as_one_character(raw: &[Value]) {
    let text = record_with(raw, "CUT abc")["text"].as_str().unwrap();
    assert!(
        text.contains("# Cut Word at the Caf\u{e9}\n"),
        "the heading is composed"
    );
    assert!(
        text.contains("› Cut Word at the Caf\u{e9}\n"),
        "the heading line is composed"
    );
    assert!(text.contains("as every caf\u{e9} owner knows."));
    assert!(
        !text.contains('\u{301}'),
        "no separate accent is left anywhere in the text"
    );
}

#[tokio::test]
async fn the_sample_chapters_keep_every_section_with_the_scores_of_their_pages() {
    let work = tempfile::tempdir().unwrap();
    let out = work.path().join("out");

    let report = build_with(&dry_run_over(&support::sample_content(), &out)).await;

    assert!(report.rejected.is_empty(), "{:?}", report.rejected);
    assert_eq!(
        fs::read_to_string(out.join("cpt/rejected.jsonl")).unwrap(),
        ""
    );
    let raw = support::read_jsonl(&out.join("cpt/raw.jsonl"));
    assert_eq!(raw.len(), 3);
    let in_document = |title: &str| {
        raw.iter()
            .find(|record| record["source"]["document"] == title)
            .unwrap_or_else(|| panic!("a section of {title:?} is kept"))
    };
    let book = in_document("Option Volatility and Pricing, chapter 1: Sample Pages");
    assert_eq!(book["quality"]["page_score"], 0.913);
    assert_eq!(book["quality"]["latex_renders"], true);
    assert_eq!(book["quality"]["code_parses"], Value::Null);
    let intuition = in_document("Quanty Sample Notes, chapter 1: Options Pricing Intuition");
    assert_eq!(intuition["quality"]["page_score"], Value::Null);
    let in_depth = in_document("Quanty Sample Notes, chapter 2: Black Scholes In Depth");
    assert_eq!(in_depth["quality"]["page_score"], Value::Null);
    assert_eq!(in_depth["quality"]["latex_renders"], true);
    assert_eq!(in_depth["quality"]["code_parses"], true);
    for record in &raw {
        assert_eq!(record["dedup"]["sha256"].as_str().unwrap().len(), 64);
    }
}
