//! What a text is made of once it is measured: atoms with a width, in lines, and the formulas
//! that the text asked for.

use eframe::egui::{Color32, text::TextFormat};

use super::parse::SpanStyle;
use crate::media::math::{MathRef, MathState};
use crate::theme::TextRole;

#[derive(Debug, Clone)]
pub(super) struct Atom {
    pub(super) kind: AtomKind,
    pub(super) width: f32,
    /// Only formulas and chips have one.
    pub(super) ascent: f32,
    pub(super) descent: f32,
    /// Room after it, if something else follows on the same line. A break drops it.
    pub(super) gap: f32,
    /// No break may fall after it: no space follows it in the text.
    pub(super) is_glued: bool,
}

impl Atom {
    pub(super) fn is_code(&self) -> bool {
        matches!(
            self.kind,
            AtomKind::Word {
                style: SpanStyle::Code,
                ..
            }
        )
    }
}

#[derive(Debug, Clone)]
pub(super) enum AtomKind {
    Word {
        text: String,
        style: SpanStyle,
        format: TextFormat,
        /// How far each character boundary is from the start of the word, so that a word that
        /// is wider than its row can be cut. There is one number more than the word has
        /// characters, and the last is the width of the word.
        edges: Vec<f32>,
    },
    Foot {
        text: String,
        format: TextFormat,
    },
    /// Until it is painted it only holds its room.
    Math {
        /// Which of the formulas of the text it is.
        formula: usize,
        is_painted: bool,
    },
    Chip {
        number: usize,
    },
}

/// What a formula was when the text was measured.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Seen {
    Loading,
    Failed,
    Ready {
        width: f32,
        ascent: f32,
        descent: f32,
    },
}

impl From<MathState> for Seen {
    fn from(state: MathState) -> Seen {
        match state {
            MathState::Loading => Seen::Loading,
            MathState::Failed => Seen::Failed,
            MathState::Ready(image) => Seen::Ready {
                width: image.width,
                ascent: image.ascent,
                descent: image.descent,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct Formula {
    pub(super) latex: String,
    /// The role of the text around it, which sets its size.
    pub(super) role: TextRole,
    pub(super) seen: Seen,
}

impl Formula {
    pub(super) fn math_ref(&self) -> MathRef<'_> {
        MathRef::inline(&self.latex, self.role)
    }
}

/// The vertical numbers of a line that holds only text.
#[derive(Debug, Clone, Copy)]
pub(super) struct LineMetrics {
    pub(super) line_height: f32,
    /// From the top of the line to the baseline.
    pub(super) baseline: f32,
    pub(super) code_ascent: f32,
    pub(super) code_descent: f32,
    /// How far a footnote mark is lifted above the baseline.
    pub(super) foot_raise: f32,
    /// The colour of a formula on this line.
    pub(super) tint: Color32,
}

#[derive(Debug, Clone)]
pub(super) struct MeasuredLine {
    /// The list marker or the footnote mark before the text. The text stands beside it on
    /// every row.
    pub(super) marker: Option<Atom>,
    pub(super) atoms: Vec<Atom>,
    /// How far the text is from the left edge, on every row of the line.
    pub(super) indent: f32,
    pub(super) starts_paragraph: bool,
    pub(super) metrics: LineMetrics,
}

impl MeasuredLine {
    /// The runs of atoms that no break may split. Each ends with the first atom that a break
    /// may fall after.
    pub(super) fn units(&self) -> impl Iterator<Item = &[Atom]> {
        self.atoms.chunk_by(|atom, _| atom.is_glued)
    }
}

pub(super) fn unit_width(unit: &[Atom]) -> f32 {
    let gaps = unit.len().saturating_sub(1);
    let inner: f32 = unit.iter().take(gaps).map(|atom| atom.gap).sum();
    inner + unit.iter().map(|atom| atom.width).sum::<f32>()
}

#[derive(Debug, Clone)]
pub(super) struct Measured {
    pub(super) lines: Vec<MeasuredLine>,
    pub(super) formulas: Vec<Formula>,
    /// Some formula is still loading, so every formula only holds its room.
    pub(super) is_waiting: bool,
    pub(super) plain: String,
    pub(super) pixels_per_point: f32,
}

impl Measured {
    /// The text cannot be made narrower than this without a cut in a word.
    pub(super) fn widest_unit(&self) -> f32 {
        self.lines
            .iter()
            .flat_map(|line| line.units())
            .map(unit_width)
            .fold(0.0, f32::max)
    }

    pub(super) fn widest_line(&self) -> f32 {
        let width_of = |line: &MeasuredLine| match line.atoms.split_last() {
            Some((last, rest)) => {
                line.indent
                    + rest.iter().map(|atom| atom.width + atom.gap).sum::<f32>()
                    + last.width
            }
            None => line.indent,
        };
        self.lines.iter().map(width_of).fold(0.0, f32::max)
    }
}
