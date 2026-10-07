//! Decides whether a model reply is good enough to save. A failed check gives one plain sentence
//! that is sent back to the model for a second try.

mod clean;
mod fault;
mod fields;
mod latex;
mod word_match;

use super::reply::{TranscribedPage, TranscribedPiece};
use super::usable_box::UsableBox;

use clean::clean_reply;
use fault::MIN_FIGURE_EXPLANATION_WORDS;

pub use fault::{PieceRef, ReplyFault};
pub(super) use fields::any_string_has_backslash;
pub(super) use word_match::{
    piece_strings, piece_word_count, text_layer_word_count, word_match, words,
};

pub(super) const ALMOST_EMPTY_WORDS: usize = 20;

/// Tidies the reply, then returns the first rule the tidied reply breaks.
///
/// The two steps are one call because the rules read a tidied reply: the table rule expects
/// lines that are already trimmed.
pub(super) fn clean_and_check(
    page: &mut TranscribedPage,
    text_layer_words: usize,
) -> Result<(), ReplyFault> {
    clean_reply(page);
    check_reply(page, text_layer_words)
}

fn check_reply(page: &TranscribedPage, text_layer_words: usize) -> Result<(), ReplyFault> {
    piece_numbers(page)?;
    no_pieces(page, text_layer_words)?;
    fields::check_strings(page)?;
    figure_explanations(page)?;
    tables(page)?;
    discussion_links(page)?;
    mid_sentence_flags(page)?;
    displayed_math_in_text(page)?;
    // Keep this rule last. A reply whose only fault is a bad rectangle has passed every other
    // rule, so it is saved as good content and only that figure's picture falls back to the
    // whole page.
    // SMELL: a test fails if this rule moves above the string checks, but nothing fails if it
    // moves above a later rule, or if another rule is put after it. A rule that runs after this
    // one is never applied to a reply with a bad rectangle, and that reply is then saved as good
    // content.
    figure_bounds(page)
}

fn piece_numbers(page: &TranscribedPage) -> Result<(), ReplyFault> {
    for (expected, piece) in (1..).zip(&page.pieces) {
        if piece.number() != expected {
            return Err(ReplyFault::PieceNumbers {
                expected,
                found: piece.number(),
            });
        }
    }
    Ok(())
}

fn no_pieces(page: &TranscribedPage, text_layer_words: usize) -> Result<(), ReplyFault> {
    if page.pieces.is_empty() && text_layer_words >= ALMOST_EMPTY_WORDS {
        return Err(ReplyFault::NoPieces { text_layer_words });
    }
    Ok(())
}

fn figure_explanations(page: &TranscribedPage) -> Result<(), ReplyFault> {
    for (index, piece) in page.pieces.iter().enumerate() {
        if let TranscribedPiece::Figure { explanation, .. } = piece {
            let words = explanation.split_whitespace().count();
            if words < MIN_FIGURE_EXPLANATION_WORDS {
                return Err(ReplyFault::ThinFigureExplanation {
                    piece: PieceRef::of(page, index),
                    words,
                });
            }
        }
    }
    Ok(())
}

fn tables(page: &TranscribedPage) -> Result<(), ReplyFault> {
    for (index, piece) in page.pieces.iter().enumerate() {
        if let TranscribedPiece::Table { markdown, .. } = piece
            && let Some(seen) = table_problem(markdown)
        {
            return Err(ReplyFault::BrokenTable {
                piece: PieceRef::of(page, index),
                seen,
            });
        }
    }
    Ok(())
}

fn table_problem(markdown: &str) -> Option<&'static str> {
    let lines: Vec<&str> = markdown.lines().collect();
    if lines.len() < 2 {
        return Some("it has fewer than two lines");
    }
    if lines
        .iter()
        .any(|line| !line.starts_with('|') || !line.ends_with('|'))
    {
        return Some("a line does not start and end with |");
    }
    let bars = |line: &str| line.matches('|').count();
    if lines.iter().any(|line| bars(line) != bars(lines[0])) {
        return Some(
            "its lines do not all have the same number of cells: most often a | was written inside a cell, an escaped one included; in math write \\mid or \\vert",
        );
    }
    let is_separator =
        lines[1].chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')) && lines[1].contains('-');
    if !is_separator {
        return Some("its second line is not a separator row such as |---|---|");
    }
    None
}

fn discussion_links(page: &TranscribedPage) -> Result<(), ReplyFault> {
    let piece_numbered = |number: u32| page.pieces.iter().find(|piece| piece.number() == number);
    for link in &page.discusses {
        let fault = |seen| ReplyFault::BadDiscussionLink {
            piece: link.piece,
            about: link.about,
            seen,
        };
        let (Some(piece), Some(about)) = (piece_numbered(link.piece), piece_numbered(link.about))
        else {
            return Err(fault("it names a piece number that does not exist"));
        };
        if !matches!(
            piece,
            TranscribedPiece::Text { .. } | TranscribedPiece::Footnote { .. }
        ) {
            return Err(fault("its piece is not a text or footnote piece"));
        }
        if !matches!(
            about,
            TranscribedPiece::Figure { .. } | TranscribedPiece::Table { .. }
        ) {
            return Err(fault("its about is not a figure or table piece"));
        }
    }
    Ok(())
}

fn mid_sentence_flags(page: &TranscribedPage) -> Result<(), ReplyFault> {
    let is_body = |piece: &&TranscribedPiece| {
        !matches!(
            piece,
            TranscribedPiece::Figure { .. }
                | TranscribedPiece::Table { .. }
                | TranscribedPiece::Footnote { .. }
        )
    };
    let is_text =
        |piece: Option<&TranscribedPiece>| matches!(piece, Some(TranscribedPiece::Text { .. }));
    if page.starts_mid_sentence && !is_text(page.pieces.iter().find(is_body)) {
        return Err(ReplyFault::StartsMidSentenceFlag);
    }
    if page.ends_mid_sentence && !is_text(page.pieces.iter().rfind(is_body)) {
        return Err(ReplyFault::EndsMidSentenceFlag);
    }
    Ok(())
}

fn displayed_math_in_text(page: &TranscribedPage) -> Result<(), ReplyFault> {
    for (index, piece) in page.pieces.iter().enumerate() {
        let text = match piece {
            TranscribedPiece::Heading { text, .. } => text,
            TranscribedPiece::Text { markdown, .. }
            | TranscribedPiece::Footnote { markdown, .. } => markdown,
            TranscribedPiece::Formula { .. }
            | TranscribedPiece::Figure { .. }
            | TranscribedPiece::Table { .. } => continue,
        };
        if let Some(seen) = displayed_math(text) {
            return Err(ReplyFault::DisplayedMathInText {
                piece: PieceRef::of(page, index),
                seen,
            });
        }
    }
    Ok(())
}

fn figure_bounds(page: &TranscribedPage) -> Result<(), ReplyFault> {
    for (index, piece) in page.pieces.iter().enumerate() {
        if let TranscribedPiece::Figure { bounds, .. } = piece
            && let Err(seen) = UsableBox::new(*bounds)
        {
            return Err(ReplyFault::BadFigureBounds {
                piece: PieceRef::of(page, index),
                seen,
            });
        }
    }
    Ok(())
}

// `\[` is not looked for: a model may write `\[1\]` for a printed "[1]". A single `$$` is not a
// fault either, because it can be printed money.
fn displayed_math(text: &str) -> Option<&'static str> {
    if let Some(first) = text.find("$$")
        && text[first + 2..].contains("$$")
    {
        return Some("a displayed formula between $$ and $$");
    }
    [
        ("\\begin{equation", "a \\begin{equation} environment"),
        ("\\begin{align", "a \\begin{align} environment"),
        ("\\begin{gather", "a \\begin{gather} environment"),
        ("\\begin{multline", "a \\begin{multline} environment"),
    ]
    .into_iter()
    .find(|(marker, _)| text.contains(marker))
    .map(|(_, seen)| seen)
}
