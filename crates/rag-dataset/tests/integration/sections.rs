//! Cuts converted chapters into sections: the real chapter of a book for every kind of piece, the
//! hand-written chapters for ids and items, and one long chapter written here for the cut.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use ocr::read_chapter;
use rag_dataset::section::{Section, sections_of};
use rag_ingestion::chapter_items;
use serde_json::{Value, json};

use crate::support;

/// The sections of the chapter in this folder, the way a build makes them.
fn sections_at(content_folder: &Path, chapter_folder: &Path) -> Vec<Section> {
    let document = support::exported_at(content_folder, chapter_folder);
    let chapter = read_chapter(chapter_folder).unwrap();
    sections_of(&document, &chapter, &chapter_items(&chapter))
}

#[test]
fn the_sample_chapters_are_cut_into_stable_sections_that_render_as_they_were_printed() {
    sections_are_stable_and_carry_the_items_of_their_chapter();
    let content = support::sample_content();
    a_book_section_renders_its_heading_line_math_formula_label_figure_note_and_provenance(
        &sections_at(&content, &support::sample_chapter()),
    );
    a_code_listing_renders_as_a_fenced_block_with_its_indentation(&sections_at(
        &content,
        &support::in_depth_chapter(),
    ));
}

fn a_book_section_renders_its_heading_line_math_formula_label_figure_note_and_provenance(
    sections: &[Section],
) {
    let section = &sections[0];
    let text = &section.markdown;

    assert!(
        text.starts_with("# Option Volatility and Pricing › Sample Pages"),
        "{text}"
    );
    assert!(
        text.contains("$C_{i|i+1:i+j-1}$"),
        "inline math is written between dollars"
    );
    assert!(
        !text.contains("\\(") && !text.contains("\\)"),
        "the converter's marks for inline math are all gone"
    );
    assert!(
        text.contains("pay Farmer Smith's asking price of \\$100,000"),
        "a dollar sign in prose is escaped, so every other dollar sign is math"
    );
    assert!(
        text.contains("$$\n\\begin{aligned}") && text.contains("\n$$\n(7.3)\n\n"),
        "a formula is a block between two lines of dollars, and its printed label follows it"
    );

    let carrier = text.find("receivable at the end of each month.").unwrap();
    let footnote = text.find("[^1]: At this point").unwrap();
    let next_paragraph = text.find("If Jerry decides to buy the land now").unwrap();
    assert!(
        carrier < footnote && footnote < next_paragraph,
        "the footnote follows the paragraph that shows its marker, and not the foot of the page"
    );
    assert_eq!(
        &text[carrier..footnote],
        "receivable at the end of each month.\n\n",
        "nothing sits between the paragraph and its footnote"
    );

    let note = "[Figure 13-4 — description (model-written): ";
    let start = text.find(note).unwrap();
    let end = start + text[start..].find("]\n\n").unwrap();
    assert!(!text[start..end].contains("Text printed in the figure"));
    assert!(text[start..end].contains("Figure 13-4 is a line graph"));
    assert!(
        text.contains("[Figure 13-16: Dividend sensitivity. — description (model-written): "),
        "a caption follows the label"
    );
    assert!(
        section
            .notes
            .contains("Figure 13-4 printed text: \"Figure 13-4\"; \"+10, +5, 0"),
        "the words printed in a figure go to the notes: {}",
        section.notes
    );
    assert!(
        section.notes.contains(
            "Formula (7.3): Likelihood of the D-vine copula. The likelihood of the D-vine copula is"
        ),
        "{}",
        section.notes
    );

    assert_eq!(section.images.len(), 3, "one picture for each figure");
    for image in &section.images {
        assert!(image.path.is_file(), "{}", image.path.display());
        assert!(image.path.to_string_lossy().ends_with("-figure.png"));
        let at_pos: String = text.chars().skip(image.pos).take(7).collect();
        assert_eq!(
            at_pos, "[Figure",
            "pos counts characters from the start of the text"
        );
    }

    assert!(section.provenance.transcribed_tokens > 0);
    assert!(section.provenance.model_written_tokens > 0);
}

fn a_code_listing_renders_as_a_fenced_block_with_its_indentation(sections: &[Section]) {
    let listing =
        fs::read_to_string(support::in_depth_chapter().join("page-num-3/11-code.txt")).unwrap();
    let listing = listing.strip_suffix('\n').unwrap();
    assert!(
        listing.contains("\n    d1 = "),
        "the listing has indents: {listing:?}"
    );
    assert!(listing.contains("\n\n    return"), "and a blank line");

    let text = &sections[0].markdown;

    assert!(
        text.contains(&format!("```python\n{listing}\n```")),
        "the listing is written exactly as it is printed: {text}"
    );
    assert!(
        text.contains("Listing 2.1: The call price in Python\n```python\n"),
        "the printed label and caption come before the listing"
    );
}

fn sections_are_stable_and_carry_the_items_of_their_chapter() {
    let content = support::sample_content();
    let expected_ids = [
        "option-volatility-and-pricing:chapter-1:s001",
        "quanty-sample-notes:chapter-1:s001",
        "quanty-sample-notes:chapter-2:s001",
    ];
    for (folder, expected_id) in support::sample_chapters().iter().zip(expected_ids) {
        let first = sections_at(&content, folder);
        let second = sections_at(&content, folder);
        assert_eq!(first, second, "the same chapter gives the same sections");
        assert_eq!(
            first.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            [expected_id]
        );

        let chapter = read_chapter(folder).unwrap();
        let stored: HashSet<_> = chapter_items(&chapter).iter().map(|item| item.id).collect();
        let carried: HashSet<_> = first
            .iter()
            .flat_map(|section| section.item_ids.iter().copied())
            .collect();
        for section in &first {
            assert!(!section.item_ids.is_empty(), "{}", section.id);
            assert!(section.item_ids.iter().all(|id| stored.contains(id)));
        }
        assert_eq!(carried, stored, "no item of {} is lost", folder.display());
    }
}

/// A paragraph of this many tokens, led by a word that tells it from the others.
fn paragraph(name: &str, tokens: usize) -> String {
    format!("{name} {}", "abc ".repeat(tokens - 1).trim_end())
}

fn write_json(file: &Path, value: &Value) {
    fs::write(file, serde_json::to_string_pretty(value).unwrap()).unwrap();
}

/// Writes a chapter of one page in `<content>/long-notes/chapter-1`. The pieces are headings and
/// paragraphs of known sizes, and one formula that a paragraph leads into.
///
/// Under the heading "Long Chapter" there are three headings of the next rank: "Section A" with
/// 3,000 tokens, "Section B" with 200, and "Section C" with about 9,850 and no heading under it.
/// "Section B" is joined to "Section C", and the two together are cut in parts. The first part
/// takes about 7,200 tokens before the paragraph that leads into the formula. The paragraph of
/// 700 tokens fits after them, and the formula of 150 does not fit after the paragraph.
fn write_long_chapter(content: &Path) -> std::path::PathBuf {
    let folder = content.join("long-notes/chapter-1");
    let page = folder.join("page-num-1");
    fs::create_dir_all(&page).unwrap();
    write_json(
        &folder.join("chapter.json"),
        &json!({
            "format-version": 1,
            "media-title": "Long Notes",
            "name": { "chapter": { "number": 1, "name": "A Long Chapter" } },
            "source-file": "chapter-1-a-long-chapter.pdf",
            "source-sha256": "10101010101010101010101010101010101010101010101010101010101010",
            "page-count": 1,
            "finished": true,
        }),
    );
    let mut pieces = Vec::new();
    let mut relationships = Vec::new();
    let mut add = |kind: &str, extension: &str, text: String, extra: Value| {
        let number = pieces.len() + 1;
        let file = format!("{number:02}-{kind}.{extension}");
        fs::write(page.join(&file), format!("{text}\n")).unwrap();
        let mut piece = json!({ "number": number, "file": file, "kind": kind });
        piece
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        pieces.push(piece);
        number
    };
    let heading = |rank: u32| json!({ "rank": rank });
    add("heading", "md", "Long Chapter".into(), heading(1));
    add("heading", "md", "Section A".into(), heading(2));
    for name in ["A1", "A2", "A3"] {
        add("text", "md", paragraph(name, 1000), json!({}));
    }
    add("heading", "md", "Section B".into(), heading(2));
    add("text", "md", paragraph("B1", 200), json!({}));
    add("heading", "md", "Section C".into(), heading(2));
    for name in ["C1", "C2", "C3", "C4", "C5"] {
        add("text", "md", paragraph(name, 1400), json!({}));
    }
    let lead_in = add("text", "md", paragraph("LEADIN", 700), json!({}));
    let formula = add(
        "formula",
        "tex",
        format!("E = mc^2{}", " + x".repeat(120)),
        json!({ "label": "(1.1)", "name": "Energy", "statement": "Energy is mass times c squared." }),
    );
    relationships.push(json!({ "kind": "introduces", "from": lead_in, "to": formula }));
    for name in ["C6", "C7"] {
        add("text", "md", paragraph(name, 1000), json!({}));
    }
    write_json(
        &page.join("page.json"),
        &json!({
            "format-version": 1,
            "page-position": 1,
            "pieces": pieces,
            "relationships": relationships,
        }),
    );
    folder
}

#[test]
fn a_long_chapter_is_cut_into_sections_of_500_to_8000_tokens() {
    let content = tempfile::tempdir().unwrap();
    let folder = write_long_chapter(content.path());

    let sections = sections_at(content.path(), &folder);

    let ids: Vec<&str> = sections.iter().map(|section| section.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "long-notes:chapter-1:s001",
            "long-notes:chapter-1:s002:p1",
            "long-notes:chapter-1:s002:p2"
        ]
    );
    let chapter = read_chapter(&folder).unwrap();
    let mut written: Vec<_> = sections.iter().flat_map(|s| s.piece_ids.clone()).collect();
    written.sort();
    let all: Vec<_> = chapter.pieces.iter().map(|piece| piece.id).collect();
    assert_eq!(
        written, all,
        "every piece is in one section or part, and in no more"
    );
    for section in &sections {
        assert!(
            section.n_tokens >= 500,
            "{}: {}",
            section.id,
            section.n_tokens
        );
        assert!(
            section.n_tokens <= 8000,
            "{}: {}",
            section.id,
            section.n_tokens
        );
    }

    let with_b = sections
        .iter()
        .find(|section| section.markdown.contains("B1 abc"))
        .unwrap();
    assert!(
        with_b.markdown.contains("## Section A") || with_b.markdown.contains("## Section C"),
        "the short section is joined to a neighbour"
    );
    assert_eq!(
        with_b.section_path,
        ["Long Chapter"],
        "the joined section sits under the heading its parts share"
    );

    let with_formula = sections
        .iter()
        .find(|section| section.markdown.contains("$$\nE = mc^2 + x"))
        .unwrap();
    assert!(with_formula.markdown.contains("LEADIN abc"));
    assert_eq!(with_formula.id, "long-notes:chapter-1:s002:p2");
    let first_part = &sections[1];
    assert!(first_part.markdown.contains("C5 abc"));
    assert!(
        !first_part.markdown.contains("LEADIN"),
        "the lead-in goes to the next part, with its formula"
    );
}
