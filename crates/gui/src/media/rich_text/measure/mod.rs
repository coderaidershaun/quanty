//! Gives every piece of a text a width: words, footnote marks, formulas and citation chips. All
//! the words of a block are laid out in one line of text, so one layout pass measures them all.

mod pieces;
mod ruler;

use eframe::egui::{self, Color32, Ui, text::LayoutJob, text::TextFormat};

use self::pieces::{LinePieces, Piece, PieceKind, split_into_pieces};
use self::ruler::{Ruler, Slot, in_one_line};
use super::atom::{Atom, AtomKind, Formula, LineMetrics, Measured, MeasuredLine, Seen};
use super::parse::Parsed;
use super::style::{TextLook, text_look};
use crate::media::math::Math;
use crate::theme::TextRole;
use crate::widgets::CitationChip;

/// The width of the stand-in for a formula that is not typeset yet: this much of the em of
/// the formula for each letter or digit of its source.
const STAND_IN_EM_PER_LETTER: f32 = 0.5;
/// A stand-in is never wider than this many letters, however long the source is.
const STAND_IN_MAX_LETTERS: usize = 40;

/// What a text waits for before it shows its formulas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WaitFor {
    OwnFormulas,
    /// A formula somewhere else is still loading, so this text waits too: the cells of a table
    /// show their formulas together.
    OthersToo,
}

struct LineContext<'a> {
    look: &'a TextLook,
    metrics: LineMetrics,
    space_width: f32,
    formulas: &'a [Formula],
    is_waiting: bool,
}

/// `math` is asked for every formula in the text.
pub(super) fn measure(
    ui: &Ui,
    parsed: &Parsed,
    cites: &[usize],
    look: &TextLook,
    math: &mut Math,
    wait: WaitFor,
) -> Measured {
    let small = text_look(TextRole::Small);
    let (mut pieces, formulas) = split_into_pieces(parsed, look, &small, math);
    // Chips with no text before them still need a line to stand on.
    if pieces.is_empty() && !cites.is_empty() {
        pieces.push(LinePieces {
            marker: None,
            pieces: Vec::new(),
            look,
            indent_gap: 0.0,
            starts_paragraph: false,
        });
    }
    let is_waiting =
        wait == WaitFor::OthersToo || formulas.iter().any(|formula| formula.seen == Seen::Loading);

    let (parts, slots) = in_one_line(&pieces);
    let ruler = Ruler::of(&ui.painter().layout_job(one_row_job(&parts))).unwrap_or_else(|| {
        tracing::debug!(
            parts = parts.len(),
            "a text was not laid out as one glyph for each character, so each word is measured alone"
        );
        Ruler::by_part(ui, &parts)
    });
    let mut slots = slots.into_iter();
    let mut lines: Vec<MeasuredLine> = pieces
        .into_iter()
        .map(|line| measure_line(ui, line, &ruler, &mut slots, &formulas, is_waiting))
        .collect();
    add_chips(ui, cites, look, &mut lines);
    Measured {
        lines,
        formulas,
        is_waiting,
        plain: parsed.plain.clone(),
        pixels_per_point: ui.ctx().pixels_per_point(),
    }
}

/// The words of a block are measured and drawn with the same settings, or a word would not be
/// as wide when drawn as it was measured.
pub(super) fn one_row_job(parts: &[(String, TextFormat)]) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;
    job.break_on_newline = false;
    for (text, format) in parts {
        job.append(text, 0.0, format.clone());
    }
    job
}

/// How wide a piece of text is when it is laid out alone, which is how a piece of a cut word is
/// drawn. It can be wider than the same characters are in the one line of text that the words
/// were measured in, because kerning ties each character to the next.
pub(super) fn alone_width(ui: &Ui) -> impl Fn(&str, &TextFormat) -> f32 + '_ {
    move |text, format| {
        let parts = [(text.to_owned(), format.clone())];
        ui.painter().layout_job(one_row_job(&parts)).size().x
    }
}

fn measure_line(
    ui: &Ui,
    line: LinePieces<'_>,
    ruler: &Ruler,
    slots: &mut impl Iterator<Item = Slot>,
    formulas: &[Formula],
    is_waiting: bool,
) -> MeasuredLine {
    let context = LineContext {
        look: line.look,
        metrics: metrics_of(ui, line.look),
        space_width: ui
            .painter()
            .fonts_mut(|fonts| fonts.glyph_width(&line.look.font, ' ')),
        formulas,
        is_waiting,
    };
    let mut atom_of = |piece: Piece| {
        let slot = piece.as_text().and_then(|_| slots.next());
        measure_piece(piece, slot.as_ref(), ruler, &context)
    };
    let marker = line.marker.map(&mut atom_of);
    let indent = marker
        .as_ref()
        .map_or(0.0, |marker| marker.width + line.indent_gap);
    let atoms = line.pieces.into_iter().map(atom_of).collect();
    MeasuredLine {
        marker,
        atoms,
        indent,
        starts_paragraph: line.starts_paragraph,
        metrics: context.metrics,
    }
}

fn measure_piece(
    piece: Piece,
    slot: Option<&Slot>,
    ruler: &Ruler,
    context: &LineContext<'_>,
) -> Atom {
    let is_spaced = piece.has_space_after;
    let (width, gap) = slot.map_or((0.0, 0.0), |slot| ruler.measure(slot));
    let kind = match piece.kind {
        PieceKind::Word {
            text,
            style,
            format,
        } => {
            let edges = slot.map_or_else(Vec::new, |slot| ruler.edges(slot, width));
            AtomKind::Word {
                text,
                style,
                format,
                edges,
            }
        }
        PieceKind::Foot { text, format } => AtomKind::Foot { text, format },
        PieceKind::Math { formula } => return context.formula_atom(formula, is_spaced),
    };
    Atom {
        kind,
        width,
        ascent: 0.0,
        descent: 0.0,
        gap,
        is_glued: !is_spaced,
    }
}

impl LineContext<'_> {
    /// Until the formula is painted, the atom is a stand-in as high as a line of text.
    fn formula_atom(&self, formula: usize, is_spaced: bool) -> Atom {
        let metrics = &self.metrics;
        let (width, ascent, descent, is_painted) = match self.formulas[formula].seen {
            Seen::Ready {
                width,
                ascent,
                descent,
            } if !self.is_waiting => (width, ascent, descent, true),
            _ => {
                let latex = &self.formulas[formula].latex;
                let width = stand_in_width(latex, self.look.math_size);
                let below = metrics.line_height - metrics.baseline;
                (width, metrics.baseline, below, false)
            }
        };
        Atom {
            kind: AtomKind::Math {
                formula,
                is_painted,
            },
            width,
            ascent,
            descent,
            gap: if is_spaced { self.space_width } else { 0.0 },
            is_glued: !is_spaced,
        }
    }
}

fn stand_in_width(latex: &str, em: f32) -> f32 {
    let letters = latex
        .chars()
        .filter(|c| c.is_alphanumeric())
        .count()
        .clamp(1, STAND_IN_MAX_LETTERS);
    STAND_IN_EM_PER_LETTER * em * letters as f32
}

fn metrics_of(ui: &Ui, look: &TextLook) -> LineMetrics {
    let ascent_and_height = |font: &egui::FontId| {
        let galley = ui
            .painter()
            .layout_no_wrap("x".to_owned(), font.clone(), Color32::WHITE);
        galley
            .rows
            .first()
            .and_then(|row| row.row.glyphs.first())
            .map_or((font.size, font.size), |glyph| {
                (glyph.font_ascent, glyph.font_height)
            })
    };
    let (ascent, height) = ascent_and_height(&look.font);
    let (code_ascent, code_height) = ascent_and_height(&look.code_font);
    LineMetrics {
        line_height: look.line_height,
        baseline: (look.line_height - height) / 2.0 + ascent,
        code_ascent,
        code_descent: code_height - code_ascent,
        foot_raise: look.foot_raise,
        tint: look.color,
    }
}

fn add_chips(ui: &Ui, cites: &[usize], look: &TextLook, lines: &mut [MeasuredLine]) {
    let Some(line) = lines.last_mut() else {
        return;
    };
    if cites.is_empty() {
        return;
    }
    if let Some(last) = line.atoms.last_mut() {
        last.is_glued = true;
        last.gap = look.small_gap;
    }
    for &number in cites {
        let size = CitationChip::new(number).size(ui);
        line.atoms.push(Atom {
            kind: AtomKind::Chip { number },
            width: size.x,
            ascent: size.y / 2.0,
            descent: size.y / 2.0,
            gap: look.small_gap,
            is_glued: false,
        });
    }
}
