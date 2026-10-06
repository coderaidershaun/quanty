//! Decides whether a model reply is good enough to save. A reply is tidied, checked against the
//! rules a good page always meets, and measured against the page's text layer. A failed check
//! returns one plain sentence that is sent back to the model for a second try.

mod clean;
mod fault;
mod fields;
mod latex;
mod word_match;

use fault::MIN_FIGURE_EXPLANATION_WORDS;

use crate::transcribe::{TranscribedPage, TranscribedPiece};

pub(crate) use clean::clean_reply;
pub use fault::{PieceRef, ReplyFault};
pub(crate) use fields::any_string_has_backslash;
pub(crate) use word_match::{
    piece_strings, piece_word_count, text_layer_word_count, word_match, words,
};

/// A page with fewer words than this in its text layer is treated as almost empty.
pub(crate) const ALMOST_EMPTY_WORDS: usize = 20;

/// Checks a reply against the rules a good page always meets.
///
/// `text_layer_words` is how many words the page's text layer holds. The first broken rule is
/// returned.
// SMELL: a reply must be cleaned before it is checked, and nothing enforces that order. The table
// rule expects lines that the cleaning has already trimmed.
pub(crate) fn check_reply(
    page: &TranscribedPage,
    text_layer_words: usize,
) -> Result<(), ReplyFault> {
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
        return Err(ReplyFault::MidSentenceFlag {
            flag: "starts-mid-sentence",
        });
    }
    if page.ends_mid_sentence && !is_text(page.pieces.iter().rfind(is_body)) {
        return Err(ReplyFault::MidSentenceFlag {
            flag: "ends-mid-sentence",
        });
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
            && let Some(seen) = bounds.problem()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::{PageBox, Symbol};
    use crate::transcribe::{CitedKind, CitedLabel, Discussion};

    fn text(number: u32, markdown: &str) -> TranscribedPiece {
        TranscribedPiece::Text {
            number,
            markdown: markdown.to_owned(),
            cites: vec![CitedLabel {
                kind: CitedKind::Equation,
                label: "(2.14)".to_owned(),
            }],
        }
    }

    fn formula(number: u32, latex: &str) -> TranscribedPiece {
        TranscribedPiece::Formula {
            number,
            latex: latex.to_owned(),
            label: Some("(2.14)".to_owned()),
            name: None,
            statement: "The price grows with the rate.".to_owned(),
            symbols: vec![Symbol {
                symbol: "r".to_owned(),
                meaning: "the rate".to_owned(),
            }],
        }
    }

    fn figure(number: u32, explanation: String) -> TranscribedPiece {
        TranscribedPiece::Figure {
            number,
            label: Some("Figure 3-2".to_owned()),
            caption: Some("Growth of the price.".to_owned()),
            printed_text: Vec::new(),
            bounds: PageBox {
                left: 100,
                top: 200,
                right: 600,
                bottom: 500,
            },
            explanation,
        }
    }

    fn table(number: u32, markdown: &str) -> TranscribedPiece {
        TranscribedPiece::Table {
            number,
            label: None,
            caption: None,
            markdown: markdown.to_owned(),
            note: None,
            summary: "Two rows of two numbers.".to_owned(),
        }
    }

    fn long_explanation() -> String {
        format!(
            "Figure 3-2, Growth of the price. {}",
            "The line rises from the left edge to the right edge. ".repeat(10)
        )
    }

    // Every shape here is correct and a careless check would reject it.
    fn good_page() -> TranscribedPage {
        TranscribedPage {
            printed_page_number: Some("12".to_owned()),
            running_header: None,
            pieces: vec![
                text(
                    1,
                    "A { alone, 50% of $5, and \\(x_{1} \\le 8\\%\\) costs \\(\\$5\\), is given by",
                ),
                formula(
                    2,
                    "\\begin{aligned} a &= 1 \\\\{b} &= \\left( d \\right) \\end{aligned}",
                ),
                formula(3, "\\begin{gathered} a \\\\ b \\end{gathered}"),
                figure(4, long_explanation()),
                table(5, "| a | b |\n|---|---|\n| 1 | 2 |"),
                text(6, "The rest."),
            ],
            discusses: vec![Discussion { piece: 6, about: 4 }],
            starts_mid_sentence: false,
            ends_mid_sentence: true,
        }
    }

    fn replace(page: &mut TranscribedPage, index: usize, piece: TranscribedPiece) {
        page.pieces[index] = piece;
    }

    #[test]
    fn reply_check_catches_broken_latex_tables_and_links() {
        assert_eq!(check_reply(&good_page(), 100), Ok(()));

        type Break = fn(&mut TranscribedPage);
        type Expect = fn(&ReplyFault) -> bool;
        let rows: Vec<(&str, Break, Expect)> = vec![
            (
                "rule 1: piece numbers skip",
                |page| replace(page, 5, text(7, "The rest.")),
                |fault| {
                    matches!(
                        fault,
                        ReplyFault::PieceNumbers {
                            expected: 6,
                            found: 7
                        }
                    )
                },
            ),
            (
                "rule 2: no pieces on a page with text",
                |page| page.pieces.clear(),
                |fault| matches!(fault, ReplyFault::NoPieces { .. }),
            ),
            (
                "rule 3: empty text",
                |page| replace(page, 5, text(6, " ")),
                |fault| matches!(fault, ReplyFault::EmptyContent { .. }),
            ),
            (
                "rule 4: a tab where \\theta lost its backslash",
                |page| replace(page, 2, formula(3, "a + \theta")),
                |fault| matches!(fault, ReplyFault::LostBackslash { .. }),
            ),
            (
                "rule 5: a \\begin without its \\end",
                |page| replace(page, 2, formula(3, "\\begin{aligned} a \\\\ b")),
                |fault| matches!(fault, ReplyFault::Unbalanced { .. }),
            ),
            (
                "rule 6: a formula wrapped in \\[",
                |page| replace(page, 2, formula(3, "\\[ a + b \\]")),
                |fault| matches!(fault, ReplyFault::MathWillNotTypeset { .. }),
            ),
            (
                "rule 6: a bare % inside a math span",
                |page| replace(page, 0, text(1, "It is \\(8% \\times 2\\) in all")),
                |fault| matches!(fault, ReplyFault::MathWillNotTypeset { .. }),
            ),
            (
                "rule 7: a thin figure explanation",
                |page| replace(page, 3, figure(4, "A line goes up.".to_owned())),
                |fault| matches!(fault, ReplyFault::ThinFigureExplanation { .. }),
            ),
            (
                "rule 8: a ragged table",
                |page| replace(page, 4, table(5, "| a | b |\n|---|---|\n| 1 | 2 | 3 |")),
                |fault| matches!(fault, ReplyFault::BrokenTable { .. }),
            ),
            (
                "rule 9: a link to a piece that is not there",
                |page| page.discusses = vec![Discussion { piece: 6, about: 9 }],
                |fault| matches!(fault, ReplyFault::BadDiscussionLink { .. }),
            ),
            (
                "rule 10: starts mid-sentence behind a formula",
                |page| {
                    page.pieces.remove(0);
                    page.pieces.insert(0, formula(1, "a"));
                    page.starts_mid_sentence = true;
                },
                |fault| matches!(fault, ReplyFault::MidSentenceFlag { .. }),
            ),
            (
                "rule 11: \\begin{align} inside a text piece",
                |page| replace(page, 5, text(6, "So \\begin{align} a \\end{align} holds")),
                |fault| matches!(fault, ReplyFault::DisplayedMathInText { .. }),
            ),
            (
                "rule 12: a rectangle in percentages",
                |page| {
                    if let TranscribedPiece::Figure { bounds, .. } = &mut page.pieces[3] {
                        *bounds = PageBox {
                            left: 4,
                            top: 3,
                            right: 90,
                            bottom: 45,
                        };
                    }
                },
                |fault| matches!(fault, ReplyFault::BadFigureBounds { .. }),
            ),
        ];
        for (name, break_it, is_expected) in rows {
            let mut page = good_page();
            break_it(&mut page);
            let fault = check_reply(&page, 100).expect_err(name);
            assert!(is_expected(&fault), "{name}: got {fault:?}");
        }
    }
}
