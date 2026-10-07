//! Checks what a panel gets when it hands stored text to the media code: the text is read as the
//! pipeline wrote it, laid out once, and a click comes back as a citation or as the source to copy.

use eframe::egui;
use eframe::egui::epaint::text::ByteRangeExt as _;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gui::contract::Intent;
use gui::media::rich_text::{self, Clicked, RichText};
use gui::testkit::{self, Host};
use gui::theme::TextRole;

/// One piece of text that was painted: where, and how each section of it was formatted.
struct Painted {
    text: String,
    right: f32,
    baseline: f32,
    sections: Vec<(String, egui::text::TextFormat)>,
}

fn painted(harness: &Harness<'_, Host>) -> Vec<Painted> {
    fn collect(shape: &egui::Shape, found: &mut Vec<Painted>) {
        match shape {
            egui::Shape::Text(text) => {
                let galley = &text.galley;
                let first = galley.rows.first().and_then(|row| row.row.glyphs.first());
                found.push(Painted {
                    text: galley.text().to_owned(),
                    right: text.pos.x + galley.size().x,
                    baseline: text.pos.y + first.map_or(0.0, |glyph| glyph.pos.y),
                    sections: galley
                        .job
                        .sections
                        .iter()
                        .map(|section| {
                            let words = section.byte_range.slice(&galley.job.text);
                            (words.to_owned(), section.format.clone())
                        })
                        .collect(),
                });
            }
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

/// Every painted section whose words hold `words`.
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

/// All the painted text with no white space, so that a line break cannot split what is asked.
fn squeezed(painted: &[Painted]) -> String {
    painted
        .iter()
        .flat_map(|piece| piece.text.chars())
        .filter(|c| !c.is_whitespace())
        .collect()
}

/// What a panel does with what `show` returns.
fn route(click: Option<Clicked>, intents: &mut Vec<Intent>) {
    match click {
        Some(Clicked::Citation(number)) => intents.push(Intent::SelectResult(number)),
        Some(Clicked::CopyText(source)) => intents.push(Intent::CopyText(source)),
        None => {}
    }
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
fn a_citation_chip_reports_its_number() {
    let mut harness = testkit::panel([600.0, 120.0], testkit::asked("q"), |ui, cx| {
        let text = RichText::new("The value of the option.", TextRole::Body).cites(&[1, 2, 3]);
        route(rich_text::show(ui, cx.media, &text), cx.intents);
    });
    harness.run();
    harness.get_by_label("Citation 3").click();
    harness.run();
    assert_eq!(harness.state().intents, vec![Intent::SelectResult(3)]);
}

/// The five-column table of the sample chapter, as the page reader stored it.
const TABLE: &str = "| | If domestic rates rise | If domestic rates fall | If foreign rates rise | If foreign rates fall |
|---|---|---|---|---|
| stock option calls will | rise | fall | not applicable | not applicable |
| stock option puts will | fall | rise | not applicable | not applicable |
| futures option calls (stock-type settlement) | fall | fall | not applicable | not applicable |
| futures option puts (stock-type settlement) | fall | fall | not applicable | not applicable |
| futures option calls (futures-type settlement) | no effect | no effect | not applicable | not applicable |
| futures option puts (futures-type settlement) | no effect | no effect | not applicable | not applicable |
| foreign currency option calls | rise | fall | fall | rise |
| foreign currency option puts | fall | rise | rise | fall |";

const LAST_HEADER: &str = "If foreign rates fall";

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

/// The table of the sample chapter in a panel as wide as `width`, with the picture saved.
fn table_in(width: f32, picture: &str) -> (egui::Rect, Vec<Painted>) {
    let mut harness = testkit::panel([width, 600.0], testkit::asked("q"), |ui, cx| {
        route(
            rich_text::table(ui, cx.media, TABLE, TextRole::Body),
            cx.intents,
        );
    });
    harness.run();
    let node = harness.get_by_label_contains(LAST_HEADER).rect();
    let shapes = painted(&harness);
    testkit::save_png(&mut harness, picture);
    (node, shapes)
}

#[test]
fn a_wide_table_scrolls_sideways_and_a_narrow_one_fits() {
    let furthest = |pieces: &[&Painted]| {
        pieces
            .iter()
            .map(|piece| piece.right)
            .fold(f32::MIN, f32::max)
    };
    let header_font = TextRole::Small.strong_font();

    let (node, shapes) = table_in(900.0, "rich-text-table-wide");
    assert!(node.width() <= 900.0 + 0.5, "the table fits its panel");
    let header: Vec<&Painted> = shapes
        .iter()
        .filter(|piece| piece.sections.iter().all(|(_, f)| f.font_id == header_font))
        .collect();
    assert!(
        squeezed_of(&header).ends_with("Ifforeignratesfall"),
        "the last header cell is drawn after the others"
    );
    let all: Vec<&Painted> = shapes.iter().collect();
    assert!(
        furthest(&all) <= node.right() + 0.5,
        "nothing of the table is scrolled out of its node"
    );

    let (narrow, shapes) = table_in(220.0, "rich-text-table-narrow");
    assert!(
        narrow.width() <= 220.0 + 0.5,
        "a narrow panel is not overflowed"
    );
    let all: Vec<&Painted> = shapes.iter().collect();
    assert!(
        furthest(&all) > narrow.right() + 0.5,
        "the columns that do not fit are scrolled out of the node, not squeezed"
    );
}

fn squeezed_of(pieces: &[&Painted]) -> String {
    pieces
        .iter()
        .flat_map(|piece| piece.text.chars())
        .filter(|c| !c.is_whitespace())
        .collect()
}
