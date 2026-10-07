//! Checks that no word of a text is lost, drawn twice or moved when the text is laid out.
//!
//! This is a fitness test because the failure is silent: a breaker that drops, repeats or
//! reorders a word still draws something that looks like a paragraph, and an answer that
//! misses a word can say the opposite of what the books say.

use std::cell::Cell;
use std::rc::Rc;

use eframe::egui;
use gui::media::rich_text::{self, RichText};
use gui::testkit;
use gui::theme::TextRole;
use proptest::prelude::*;

/// Taller than any text that is made here, because lines that are below the panel are not drawn.
const PANEL_HEIGHT: f32 = 3000.0;

#[derive(Debug, Clone, Copy)]
enum Mark {
    Plain,
    Emphasis,
    Strong,
    Code,
}

/// One or more words, written with the same mark around them.
#[derive(Debug, Clone)]
struct Phrase {
    words: Vec<String>,
    mark: Mark,
}

impl Phrase {
    fn written(&self) -> String {
        let words = self.words.join(" ");
        match self.mark {
            Mark::Plain => words,
            Mark::Emphasis => format!("*{words}*"),
            Mark::Strong => format!("**{words}**"),
            Mark::Code => format!("`{words}`"),
        }
    }
}

fn phrase() -> impl Strategy<Value = Phrase> {
    let mark = prop_oneof![
        4 => Just(Mark::Plain),
        1 => Just(Mark::Emphasis),
        1 => Just(Mark::Strong),
        1 => Just(Mark::Code),
    ];
    (
        prop::collection::vec("[A-Za-z0-9éèüñ–—][A-Za-z0-9éèüñ–—-]{0,11}", 1..4),
        mark,
    )
        .prop_map(|(words, mark)| Phrase { words, mark })
}

/// Phrases with a space, a line break or a blank line between them.
fn text() -> impl Strategy<Value = (String, String)> {
    let separator = prop_oneof![
        6 => Just(" "),
        1 => Just("\n"),
        1 => Just("\n\n"),
    ];
    prop::collection::vec((phrase(), separator), 1..30).prop_map(|parts| {
        let mut written = String::new();
        let mut words = String::new();
        for (index, (phrase, separator)) in parts.iter().enumerate() {
            if index > 0 {
                written.push_str(separator);
            }
            written.push_str(&phrase.written());
            words.extend(phrase.words.iter().map(String::as_str));
        }
        (written, words)
    })
}

/// The text of every piece of text that was painted, in paint order, with where it ends.
fn painted(shape: &egui::Shape, found: &mut Vec<(String, f32)>) {
    match shape {
        egui::Shape::Text(text) => {
            let right = text.pos.x + text.galley.size().x;
            found.push((text.galley.text().to_owned(), right));
        }
        egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| painted(shape, found)),
        _ => {}
    }
}

/// What is wrong with a text laid out in a panel `width` wide, if anything: the words that are
/// drawn are not `words` in order, or a piece of text ends past the width.
fn what_went_wrong(written: &str, words: &str, width: f32) -> Option<String> {
    let written = written.to_owned();
    let left = Rc::new(Cell::new(0.0));
    let seen_left = Rc::clone(&left);
    let mut harness = testkit::panel([width, PANEL_HEIGHT], testkit::asked("q"), move |ui, cx| {
        seen_left.set(ui.max_rect().left());
        let text = RichText::new(&written, TextRole::Body);
        // A text with no citations has nothing to click, and this test copies nothing.
        drop(rich_text::show(ui, cx.media, &text));
    });
    harness.run();

    let mut found = Vec::new();
    for clipped in &harness.output().shapes {
        painted(&clipped.shape, &mut found);
    }
    let drawn: String = found
        .iter()
        .flat_map(|(text, _)| text.chars())
        .filter(|c| !c.is_whitespace())
        .collect();
    if drawn != words {
        return Some(format!("`{drawn}` was drawn and `{words}` was written"));
    }
    found.iter().find_map(|(text, right)| {
        (*right > left.get() + width + 1.0).then(|| {
            format!(
                "`{text}` ends at {right} and the width ends at {}",
                left.get() + width
            )
        })
    })
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn every_word_is_drawn_once_in_order_and_inside_the_width(
        (written, words) in text(),
        width in 60.0f32..600.0,
    ) {
        if let Some(problem) = what_went_wrong(&written, &words, width) {
            return Err(TestCaseError::fail(problem));
        }
    }
}

/// A case that the property above found. The last word is wider than its row, and the piece of it
/// that ends in K was drawn past the width: in one line the K is kerned with the dash after it,
/// and drawn alone it is not.
#[test]
fn a_word_wider_than_its_row_is_cut_inside_the_width() {
    let written = "e8ñAèdMèA Aü eü ñsè3bñèh ñièuüè 1- uñ2Ea èi1WEññè ü1ü1 5GpV6 e-Eñv ñèèChmñ `aaü–` Wx5èñ0 –C-U-ñüè Mñbpe6ñp rüñAC MèADEñüh –Mè- ZèXMAü-A ie6ñG-ñèY `aAAü0ü–A–ñA A ñ` 0iK2übvü —F0P—-K-";
    let words: String = written
        .chars()
        .filter(|c| *c != '`' && !c.is_whitespace())
        .collect();
    if let Some(problem) = what_went_wrong(written, &words, 68.2227) {
        panic!("{problem}");
    }
}
