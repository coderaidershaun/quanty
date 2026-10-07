//! The faults a reply check can find, each with the sentence the model is told.

use std::fmt;

use crate::convert::reply::{TranscribedPage, TranscribedPiece};
use crate::convert::usable_box::MIN_FIGURE_SIDE;

pub(super) const MIN_FIGURE_EXPLANATION_WORDS: usize = 60;

/// Names a piece by number, kind, and label or first six words. The second try is a fresh session
/// that never saw the first reply, so a number alone would mean nothing to it.
#[derive(Debug, Clone, PartialEq)]
pub struct PieceRef {
    pub number: u32,
    pub kind: &'static str,
    pub called: String,
}

impl PieceRef {
    pub(super) fn of(page: &TranscribedPage, index: usize) -> Self {
        let piece = &page.pieces[index];
        let (label, description): (Option<&str>, &str) = match piece {
            TranscribedPiece::Heading { text, .. } => (None, text),
            TranscribedPiece::Text { markdown, .. }
            | TranscribedPiece::Footnote { markdown, .. } => (None, markdown),
            TranscribedPiece::Formula {
                label, statement, ..
            } => (label.as_deref(), statement),
            TranscribedPiece::Figure {
                label, explanation, ..
            } => (label.as_deref(), explanation),
            TranscribedPiece::Table { label, summary, .. } => (label.as_deref(), summary),
        };
        let called = match label {
            Some(label) => format!("\"{label}\""),
            None => {
                let first_words: Vec<&str> = description.split_whitespace().take(6).collect();
                format!("beginning \"{}\"", first_words.join(" "))
            }
        };
        Self {
            number: piece.number(),
            kind: piece.kind_name(),
            called,
        }
    }
}

impl fmt::Display for PieceRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "piece {}, the {} {}",
            self.number, self.kind, self.called
        )
    }
}

/// The first rule a reply broke. Its message is the sentence the model is told.
#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum ReplyFault {
    #[error(
        "the pieces are not numbered 1, 2, 3 in order: piece number {expected} was expected but {found} was found"
    )]
    PieceNumbers { expected: u32, found: u32 },

    #[error(
        "the reply has no pieces, but the page's text layer holds {text_layer_words} words; return every piece printed on the page, and no pieces only for a page with nothing printed on it"
    )]
    NoPieces { text_layer_words: usize },

    #[error("{piece} has an empty {field}")]
    EmptyContent {
        piece: PieceRef,
        field: &'static str,
    },

    #[error(
        "{} holds {seen}: a LaTeX command lost its backslash, or LaTeX was written over several lines, or a tab was used for spacing",
        location(.piece.as_ref(), .field)
    )]
    LostBackslash {
        /// `None` when the string is a page field.
        piece: Option<PieceRef>,
        field: &'static str,
        seen: &'static str,
    },

    #[error(
        "{} uses aligned but has no row break: a row break lost a backslash, or the formula is one line and needs no aligned",
        location(.piece.as_ref(), .field)
    )]
    AlignedWithoutRowBreak {
        /// `None` when the string is a page field.
        piece: Option<PieceRef>,
        field: &'static str,
    },

    #[error("{} holds {seen}", location(Some(.piece), .field))]
    Unbalanced {
        piece: PieceRef,
        field: &'static str,
        seen: &'static str,
    },

    #[error("{} would not typeset as math: {seen}", location(Some(.piece), .field))]
    MathWillNotTypeset {
        piece: PieceRef,
        field: &'static str,
        seen: &'static str,
    },

    #[error(
        "{piece} has an explanation of only {words} words; describe the figure in detail, in at least {MIN_FIGURE_EXPLANATION_WORDS} words: what it is, its axes or parts, each curve or part, each annotation quoted in full, and what it demonstrates"
    )]
    ThinFigureExplanation { piece: PieceRef, words: usize },

    #[error("{piece} has a table that is not valid Markdown: {seen}")]
    BrokenTable { piece: PieceRef, seen: &'static str },

    #[error("the discusses entry (piece {piece}, about {about}) is wrong: {seen}")]
    BadDiscussionLink {
        piece: u32,
        about: u32,
        seen: &'static str,
    },

    #[error(
        "starts-mid-sentence is true, but the first piece that is not a figure, table or footnote is not a text piece; make it false unless the page really begins in the middle of a sentence in a text piece"
    )]
    StartsMidSentenceFlag,

    #[error(
        "ends-mid-sentence is true, but the last piece that is not a figure, table or footnote is not a text piece; make it false unless the page really ends in the middle of a sentence in a text piece"
    )]
    EndsMidSentenceFlag,

    #[error(
        "{piece} holds {seen} inside its text; a displayed formula must be a formula piece of its own, with its label and statement"
    )]
    DisplayedMathInText { piece: PieceRef, seen: &'static str },

    #[error(
        "{piece} has bounds that cannot be right: {seen}; give left, top, right and bottom as whole numbers from 0 to 1000, measured from the very edges of the page picture, left and right in thousandths of the picture's width from its left edge, top and bottom in thousandths of the picture's height from its top edge, with left less than right, top less than bottom, and at least {MIN_FIGURE_SIDE} between each pair, around the whole figure"
    )]
    BadFigureBounds { piece: PieceRef, seen: &'static str },

    #[error(
        "the whole picture is one figure, so the reply must hold exactly one figure piece, but it holds {found}; describe the whole picture in that one piece"
    )]
    NotOneFigure { found: usize },
}

fn location(piece: Option<&PieceRef>, field: &str) -> String {
    match piece {
        Some(piece) => format!("the {field} of {piece}"),
        None => format!("the page's {field}"),
    }
}
