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
        prop_assert_eq!(drawn, words);
        for (text, right) in &found {
            prop_assert!(
                *right <= left.get() + width + 1.0,
                "`{}` ends at {} and the width ends at {}", text, right, left.get() + width
            );
        }
    }
}
