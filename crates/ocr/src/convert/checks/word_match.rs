//! Compares the words a model copied with the words in the page's text layer. A copy that was
//! summarised, cut short or padded scores low in one direction or the other.

use std::collections::HashMap;

use super::clean::remove_invisible;
use crate::content::WordMatch;
use crate::convert::reply::{TranscribedPage, TranscribedPiece};

pub(crate) fn words(text: &str) -> Vec<String> {
    remove_invisible(text)
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

pub(crate) fn text_layer_word_count(text_layer: &str) -> usize {
    words(text_layer).len()
}

pub(crate) fn piece_word_count(page: &TranscribedPage) -> usize {
    page.pieces
        .iter()
        .flat_map(|piece| match piece {
            TranscribedPiece::Heading { text, .. } => vec![text.as_str()],
            TranscribedPiece::Text { markdown, .. }
            | TranscribedPiece::Footnote { markdown, .. } => vec![markdown.as_str()],
            TranscribedPiece::Table { markdown, note, .. } => std::iter::once(markdown.as_str())
                .chain(note.as_deref())
                .collect(),
            TranscribedPiece::Formula { .. } | TranscribedPiece::Figure { .. } => Vec::new(),
        })
        .map(|text| words(text).len())
        .sum()
}

fn copied_strings(page: &TranscribedPage) -> Vec<&str> {
    let mut copied: Vec<&str> = Vec::new();
    copied.extend(page.printed_page_number.as_deref());
    copied.extend(page.running_header.as_deref());
    copied.extend(page.pieces.iter().flat_map(piece_strings));
    copied
}

/// The strings one piece copied from the page, not what the model wrote in its own words.
pub(crate) fn piece_strings(piece: &TranscribedPiece) -> Vec<&str> {
    let mut copied: Vec<&str> = Vec::new();
    match piece {
        TranscribedPiece::Heading {
            printed_number,
            text,
            ..
        } => {
            copied.extend(printed_number.as_deref());
            copied.push(text);
        }
        TranscribedPiece::Text { markdown, .. } => copied.push(markdown),
        // SMELL: a formula's printed label is copied from the page too, but it is left out
        // here, so a page with labelled formulas matches a little less of its text layer.
        // Adding it would change the ratios, which are already saved for converted pages.
        TranscribedPiece::Formula { .. } => {}
        TranscribedPiece::Figure {
            label,
            caption,
            printed_text,
            ..
        } => {
            copied.extend(label.as_deref());
            copied.extend(caption.as_deref());
            copied.extend(printed_text.iter().map(String::as_str));
        }
        TranscribedPiece::Table {
            label,
            caption,
            markdown,
            note,
            ..
        } => {
            copied.extend(label.as_deref());
            copied.extend(caption.as_deref());
            copied.push(markdown);
            copied.extend(note.as_deref());
        }
        TranscribedPiece::Footnote {
            marker, markdown, ..
        } => {
            copied.extend(marker.as_deref());
            copied.push(markdown);
        }
    }
    copied
}

fn counts(words: &[String]) -> HashMap<&str, usize> {
    let mut counts = HashMap::new();
    for word in words {
        *counts.entry(word.as_str()).or_insert(0) += 1;
    }
    counts
}

pub(crate) fn word_match(page: &TranscribedPage, text_layer: &str) -> WordMatch {
    let page_words = words(&copied_strings(page).join(" "));
    let layer_words = words(text_layer);
    let layer_counts = counts(&layer_words);
    let overlap: usize = counts(&page_words)
        .into_iter()
        .map(|(word, count)| count.min(layer_counts.get(word).copied().unwrap_or(0)))
        .sum();
    // Two empty sides match perfectly; one empty side matches nothing.
    if page_words.is_empty() || layer_words.is_empty() {
        let both = if page_words.is_empty() && layer_words.is_empty() {
            1.0
        } else {
            0.0
        };
        return WordMatch {
            piece_words_in_text_layer: both,
            text_layer_words_in_pieces: both,
        };
    }
    let ratio = |side: usize| (overlap as f64 / side as f64 * 1000.0).round() / 1000.0;
    WordMatch {
        piece_words_in_text_layer: ratio(page_words.len()),
        text_layer_words_in_pieces: ratio(layer_words.len()),
    }
}
