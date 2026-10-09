//! Cuts the spans of a text into words, footnote marks and formulas, and says where a space
//! comes after one. A formula that cannot be typeset becomes the words of its source.

use eframe::egui::text::TextFormat;

use crate::media::math::{Math, MathRef};
use crate::media::rich_text::atom::{Formula, Seen};
use crate::media::rich_text::parse::{LineKind, Parsed, Span, SpanStyle};
use crate::media::rich_text::style::TextLook;

/// A word, a mark or a formula, before it has a width.
pub(super) struct Piece {
    pub(super) kind: PieceKind,
    pub(super) has_space_after: bool,
}

pub(super) enum PieceKind {
    Word {
        text: String,
        style: SpanStyle,
        format: TextFormat,
    },
    Foot {
        text: String,
        format: TextFormat,
    },
    Math {
        formula: usize,
    },
}

impl Piece {
    pub(super) fn as_text(&self) -> Option<(&str, &TextFormat)> {
        match &self.kind {
            PieceKind::Word { text, format, .. } | PieceKind::Foot { text, format } => {
                Some((text, format))
            }
            PieceKind::Math { .. } => None,
        }
    }
}

pub(super) struct LinePieces<'a> {
    pub(super) marker: Option<Piece>,
    pub(super) pieces: Vec<Piece>,
    pub(super) look: &'a TextLook,
    /// Room between the marker and the text.
    pub(super) indent_gap: f32,
    pub(super) starts_paragraph: bool,
}

/// `math` is asked for every formula. `small` is the look of a footnote note, whatever the look
/// of the text is.
pub(super) fn split_into_pieces<'a>(
    parsed: &Parsed,
    look: &'a TextLook,
    small: &'a TextLook,
    math: &mut Math,
) -> (Vec<LinePieces<'a>>, Vec<Formula>) {
    let mut lines = Vec::new();
    let mut formulas = Vec::new();
    for line in &parsed.lines {
        let (look, marker, indent_gap) = match &line.kind {
            LineKind::Text => (look, None, 0.0),
            LineKind::ListItem { marker } => {
                let format = look.format(SpanStyle::Plain);
                let piece = word_piece(marker, SpanStyle::Plain, format);
                (look, Some(piece), look.list_gap)
            }
            LineKind::Note { mark } => (small, Some(foot_piece(mark, small)), small.small_gap),
        };
        let mut pieces = Vec::new();
        for span in &line.spans {
            push_span(&mut pieces, span, look, math, &mut formulas);
        }
        lines.push(LinePieces {
            marker,
            pieces,
            look,
            indent_gap,
            starts_paragraph: line.starts_paragraph,
        });
    }
    (lines, formulas)
}

fn word_piece(text: &str, style: SpanStyle, format: TextFormat) -> Piece {
    Piece {
        kind: PieceKind::Word {
            text: text.to_owned(),
            style,
            format,
        },
        has_space_after: false,
    }
}

fn foot_piece(mark: &str, look: &TextLook) -> Piece {
    Piece {
        kind: PieceKind::Foot {
            text: mark.to_owned(),
            format: look.foot_format(),
        },
        has_space_after: false,
    }
}

fn push_span(
    pieces: &mut Vec<Piece>,
    span: &Span,
    look: &TextLook,
    math: &mut Math,
    formulas: &mut Vec<Formula>,
) {
    match span {
        Span::Text { text, style } => push_words(pieces, text, *style, look),
        Span::Foot(mark) => pieces.push(foot_piece(mark, look)),
        Span::Math(latex) => {
            let seen = Seen::from(math.get(&MathRef::inline(latex, look.role)));
            let formula = formulas.len();
            formulas.push(Formula {
                latex: latex.clone(),
                role: look.role,
                seen,
            });
            if seen == Seen::Failed {
                push_words(pieces, latex, SpanStyle::Code, look);
            } else {
                pieces.push(Piece {
                    kind: PieceKind::Math { formula },
                    has_space_after: false,
                });
            }
        }
    }
}

/// A space that may break a line. A no-break space is part of a word.
fn is_space(c: char) -> bool {
    c.is_whitespace() && !matches!(c, '\u{a0}' | '\u{2007}' | '\u{202f}')
}

fn push_words(pieces: &mut Vec<Piece>, text: &str, style: SpanStyle, look: &TextLook) {
    if text.starts_with(is_space)
        && let Some(last) = pieces.last_mut()
    {
        last.has_space_after = true;
    }
    let ends_with_space = text.ends_with(is_space);
    let words: Vec<&str> = text
        .split(is_space)
        .filter(|word| !word.is_empty())
        .collect();
    for (index, word) in words.iter().enumerate() {
        let mut piece = word_piece(word, style, look.format(style));
        piece.has_space_after = index + 1 < words.len() || ends_with_space;
        pieces.push(piece);
    }
}
