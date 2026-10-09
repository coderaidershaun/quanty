//! Checks blocks of stored text: they are read as the pipeline writes them, laid out once more
//! when their formulas settle, and copied as their stored source.

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gui::contract::Intent;
use gui::media::math::{MathImage, MathRef, MathState};
use gui::media::rich_text::{self, RichText};
use gui::testkit::{self, Host};
use gui::theme::TextRole;

use super::{LAST_HEADER, Painted, TABLE, painted, route, squeezed};

fn pictures(harness: &Harness<'_, Host>) -> Vec<(egui::TextureId, egui::Rect)> {
    fn collect(shape: &egui::Shape, found: &mut Vec<(egui::TextureId, egui::Rect)>) {
        match shape {
            egui::Shape::Mesh(mesh) => found.push((mesh.texture_id, mesh.calc_bounds())),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| collect(shape, found)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    for clipped in &harness.output().shapes {
        collect(&clipped.shape, &mut found);
    }
    found
}

/// The texture has a clear margin around the box, and the picture may be drawn smaller than it
/// was made.
fn baseline_of(image: &MathImage, covered: egui::Rect) -> f32 {
    let scale = covered.height() / image.texture_size.y;
    covered.bottom() - (image.descent + image.bleed) * scale
}

fn sections_with<'a>(
    painted: &'a [Painted],
    words: &str,
) -> Vec<&'a (String, egui::text::TextFormat)> {
    painted
        .iter()
        .flat_map(|piece| piece.sections.iter())
        .filter(|(text, _)| text.contains(words))
        .collect()
}

/// One text with every rule of the stored dialect: emphasis, strong and both, code, formulas,
/// escaped marks, marks that open nothing, a soft hyphen and a tab, the four list markers, and
/// a formula that cannot be typeset.
const GRAMMAR: &str = concat!(
    r"The *delta* is **strong**, ***both*** and `code` with \(x^2\) and \(\frac{\partial V}{\partial t}\) in a line.",
    "\n",
    r"Pay $100,000 for 2 * 3, or \*plain\* [text](url), \` and \[, a_b, # and an open \(x.",
    "\nA soft hy\u{ad}phen goes and a\ttab is a space.\n",
    "1. First item\n- Second item\n• Third item\n* Fourth item\n",
    r"A formula that is broken, \(\frac{a\), shows its source.",
);

const GRAMMAR_PLAIN: &str = concat!(
    r"The delta is strong, both and code with x^2 and \frac{\partial V}{\partial t} in a line.",
    "\n",
    r"Pay $100,000 for 2 * 3, or *plain* [text](url), ` and [, a_b, # and an open \(x.",
    "\nA soft hyphen goes and a tab is a space.\n",
    "1. First item\n- Second item\n• Third item\n* Fourth item\n",
    r"A formula that is broken, \frac{a, shows its source.",
);

const CHUNK: &str = "A hedge works on the one hand and fails on the other.[^2] A position is the sum of its parts.\n\n[^2]: The right amount changes with the price.";

const CHUNK_PLAIN: &str = "A hedge works on the one hand and fails on the other. A position is the sum of its parts.\n\nThe right amount changes with the price.";

#[test]
fn stored_text_is_read_as_the_pipeline_writes_it() {
    let mut harness = testkit::panel([700.0, 360.0], testkit::asked("q"), |ui, cx| {
        for markdown in [GRAMMAR, CHUNK] {
            let text = RichText::new(markdown, TextRole::Body);
            route(rich_text::show(ui, cx.media, &text), cx.intents);
            ui.add_space(12.0);
        }
    });
    harness.run();
    harness.get_by_label(GRAMMAR_PLAIN);
    harness.get_by_label(CHUNK_PLAIN);

    let shapes = painted(&harness);
    let strong = TextRole::Body.strong_font();
    let code = TextRole::Body.code_font();
    assert!(
        sections_with(&shapes, "delta")
            .iter()
            .any(|(_, format)| format.italics),
        "an emphasis is slanted"
    );
    assert!(
        sections_with(&shapes, "strong")
            .iter()
            .any(|(_, format)| format.font_id == strong),
        "a strong word is in the strong font"
    );
    assert!(
        sections_with(&shapes, "both")
            .iter()
            .any(|(_, format)| format.italics && format.font_id == strong),
        "three stars are both slanted and strong"
    );
    assert!(
        sections_with(&shapes, "code")
            .iter()
            .any(|(text, format)| format.font_id == code && !text.contains("with")),
        "a code span is in the code font and nothing else is"
    );
    let all = squeezed(&shapes);
    for literal in [
        "$100,000",
        "2*3",
        r"\(x.",
        "*plain*",
        "[text](url)",
        "`and[,",
        "a_b,#and",
        "softhyphengoes",
        "1.Firstitem-Seconditem•Thirditem*Fourthitem",
    ] {
        assert!(
            all.contains(literal),
            "`{literal}` is drawn as it is written: {all}"
        );
    }
    for mark in ["**", "`code", "code`", "[^", "]:", r"\*", "\u{ad}"] {
        assert!(!all.contains(mark), "`{mark}` is never drawn: {all}");
    }

    let marks: Vec<&Painted> = shapes.iter().filter(|piece| piece.text == "2").collect();
    assert_eq!(
        marks.len(),
        2,
        "the mark is drawn in the text and before its note"
    );
    for mark in &marks {
        assert!(mark.sections[0].1.font_id.size < TextRole::Body.font().size);
    }
    let other = shapes
        .iter()
        .find(|piece| piece.text.contains("on the other."))
        .expect("the words before the mark");
    assert!(
        marks[0].baseline < other.baseline - 1.0,
        "the mark is raised above the baseline of the words"
    );
    let note = shapes
        .iter()
        .find(|piece| piece.text.contains("amount"))
        .expect("the words of the note");
    assert!(
        marks[1].baseline < note.baseline - 1.0,
        "the mark before the note is raised too"
    );
    assert!(
        note.sections
            .iter()
            .all(|(_, format)| format.font_id == TextRole::Small.font()),
        "a note is set in small type"
    );

    harness.state_mut().media.run_pending();
    harness.run();
    harness.get_by_label(GRAMMAR_PLAIN);
    let settled = painted(&harness);
    assert!(
        sections_with(&settled, r"\frac{a")
            .iter()
            .any(|(_, format)| format.font_id == code),
        "a formula that cannot be typeset is drawn as its source, in the code font"
    );
    testkit::save_png(&mut harness, "rich-text-ready");
}

#[test]
fn copy_text_returns_the_stored_source() {
    const COPIED: &str = r"The *delta* of \(x\) is **risk**.";
    const COPIED_PLAIN: &str = r"The delta of x is risk.";
    let mut harness = testkit::panel([600.0, 120.0], testkit::asked("q"), |ui, cx| {
        let text = RichText::new(COPIED, TextRole::Body);
        route(rich_text::show(ui, cx.media, &text), cx.intents);
    });
    harness.run();
    harness.get_by_label(COPIED_PLAIN).click_secondary();
    harness.run();
    harness.get_by_label("Copy text").click();
    harness.run();
    assert_eq!(
        harness.state().intents,
        vec![Intent::CopyText(COPIED.to_owned())],
        "the stored source is copied, not the text that was drawn"
    );

    for width in [900.0, 300.0] {
        let mut harness = testkit::panel([width, 700.0], testkit::asked("q"), |ui, cx| {
            let table = rich_text::table(ui, cx.media, TABLE, TextRole::Body);
            route(table, cx.intents);
        });
        harness.run();
        harness.get_by_label_contains(LAST_HEADER).click_secondary();
        harness.run();
        harness.get_by_label("Copy table").click();
        harness.run();
        assert_eq!(
            harness.state().intents,
            vec![Intent::CopyText(TABLE.to_owned())],
            "the table is copied as it is stored, in a panel {width} points wide"
        );
    }
}

#[test]
fn a_block_is_laid_out_once_more_when_its_formulas_settle() {
    const TEXT: &str =
        r"The price \(S\) moves with \(\sigma\sqrt{T}\) and the rate \(r\) over the period.";
    const PLAIN: &str = r"The price S moves with \sigma\sqrt{T} and the rate r over the period.";
    let mut harness = testkit::panel([460.0, 120.0], testkit::asked("q"), |ui, cx| {
        let text = RichText::new(TEXT, TextRole::Body);
        route(rich_text::show(ui, cx.media, &text), cx.intents);
    });
    harness.run();
    let stats = |harness: &Harness<'_, Host>| harness.state().media.text.stats();
    assert_eq!(
        stats(&harness).layouts_built,
        1,
        "one layout, with gaps for the formulas"
    );
    testkit::save_png(&mut harness, "rich-text-loading");

    harness.state_mut().media.run_pending();
    harness.run();
    assert_eq!(
        stats(&harness).layouts_built,
        2,
        "one more layout, with the formulas"
    );
    let settled = harness.get_by_label(PLAIN).rect();

    for latex in ["S", r"\sigma\sqrt{T}", "r"] {
        let math = MathRef::inline(latex, TextRole::Body);
        let image = match harness.state_mut().media.math.get(&math) {
            MathState::Ready(image) => image,
            other => panic!("`{latex}` should be typeset by now, and it is {other:?}"),
        };
        let covered = pictures(&harness)
            .into_iter()
            .find_map(|(texture, covered)| (texture == image.texture).then_some(covered))
            .unwrap_or_else(|| panic!("`{latex}` should be painted"));
        let box_left = covered.left() + image.bleed;
        let words = painted(&harness)
            .into_iter()
            .filter(|piece| piece.right <= box_left + 1.0)
            .max_by(|a, b| a.right.total_cmp(&b.right))
            .unwrap_or_else(|| panic!("`{latex}` should have words in front of it"));
        let drift = baseline_of(&image, covered) - words.baseline;
        assert!(
            drift.abs() <= 0.5,
            "`{latex}` is {drift} points off the baseline of `{}`",
            words.text
        );
    }

    for _ in 0..20 {
        harness.step();
    }
    assert_eq!(
        stats(&harness).layouts_built,
        2,
        "nothing is laid out again"
    );
    assert_eq!(stats(&harness).texts_parsed, 1, "nothing is read again");
    assert_eq!(
        harness.get_by_label(PLAIN).rect(),
        settled,
        "the text stays where it is"
    );
}
