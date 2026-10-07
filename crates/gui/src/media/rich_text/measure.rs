//! Gives every piece of a text a width: words, footnote marks, formulas and citation chips. All
//! the words of a block are laid out in one line of text, so one layout pass measures them all.

use eframe::egui::{self, Color32, Ui, text::LayoutJob, text::TextFormat};

use super::atom::{Atom, AtomKind, Formula, LineMetrics, Measured, MeasuredLine, Seen};
use super::parse::{Parsed, SpanStyle};
use super::pieces::{LinePieces, Piece, PieceKind, split_into_pieces};
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
    /// Only its own formulas.
    OwnFormulas,
    /// A formula somewhere else is still loading, so this text waits too: the cells of a table
    /// show their formulas together.
    OthersToo,
}

/// What the atoms of one line need to know about the line.
struct LineContext<'a> {
    look: &'a TextLook,
    metrics: LineMetrics,
    space_width: f32,
    formulas: &'a [Formula],
    is_waiting: bool,
}

/// Measures a text. `math` is asked for every formula in it.
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

/// A layout job that sets its parts in one row. The words of a block are measured and drawn
/// with the same settings, or a word would not be as wide when drawn as it was measured.
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

/// All the words of the text, in the order of the one line of text that they make, and where
/// each piece sits in it.
fn in_one_line(lines: &[LinePieces<'_>]) -> (Vec<(String, TextFormat)>, Vec<Slot>) {
    let mut parts = Vec::new();
    let mut slots = Vec::new();
    let mut glyphs = 0;
    for line in lines {
        let plain = line.look.format(SpanStyle::Plain);
        // A marker has a space after it in the one line of text, so that it is measured apart
        // from the first word.
        let marker = line.marker.iter().map(|marker| (marker, true));
        let pieces = line
            .pieces
            .iter()
            .map(|piece| (piece, piece.has_space_after));
        let texts: Vec<(&str, &TextFormat, bool)> = marker
            .chain(pieces)
            .filter_map(|(piece, is_spaced)| {
                let (text, format) = piece.as_text()?;
                Some((text, format, is_spaced))
            })
            .collect();
        for (index, &(text, format, is_spaced)) in texts.iter().enumerate() {
            let is_last = index + 1 == texts.len();
            // The last piece of a line gets a space too, so that it never touches the next line.
            let space = if is_spaced || is_last { " " } else { "" };
            // A space is set in the plain font unless the next word is in the same font as this
            // one, so that a code word is not followed by the wide space of the code font.
            let next_font = texts.get(index + 1).map(|(_, next, _)| &next.font_id);
            let space_format = if next_font == Some(&format.font_id) {
                format
            } else {
                &plain
            };
            parts.push((text.to_owned(), format.clone()));
            if !space.is_empty() {
                parts.push((space.to_owned(), space_format.clone()));
            }
            let chars = text.chars().count();
            let after = glyphs + chars + space.len();
            slots.push(Slot {
                start: glyphs,
                chars,
                is_spaced,
                is_joined: !is_spaced && !is_last,
                after,
            });
            glyphs = after;
        }
    }
    (parts, slots)
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
    /// The atom of a formula. Until the formula is painted, the atom is a stand-in as high as a
    /// line of text.
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

/// The room that is kept for a formula until it is typeset.
fn stand_in_width(latex: &str, em: f32) -> f32 {
    let letters = latex
        .chars()
        .filter(|c| c.is_alphanumeric())
        .count()
        .clamp(1, STAND_IN_MAX_LETTERS);
    STAND_IN_EM_PER_LETTER * em * letters as f32
}

/// The height of a line of text in this look, and where its baseline is.
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

/// The chips of the citations stand after the last word. The first one sticks to it.
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

/// Where a piece sits among the glyphs of the one line of text.
struct Slot {
    start: usize,
    chars: usize,
    /// A space of the text follows it, or it is a marker.
    is_spaced: bool,
    /// The next piece of the same line starts right after it.
    is_joined: bool,
    /// The first glyph after the piece and the space that follows it, if any.
    after: usize,
}

/// The positions of the glyphs of the one line of text.
struct Ruler {
    lefts: Vec<f32>,
    rights: Vec<f32>,
    end: f32,
}

impl Ruler {
    /// The positions that the layout gave, when it made one glyph for each character. Some
    /// scripts that run right to left make more, and then no position can be told from the
    /// number of a character.
    fn of(galley: &egui::Galley) -> Option<Ruler> {
        let glyphs = galley.rows.first().map_or(&[][..], |row| &row.row.glyphs);
        if glyphs.len() != galley.job.text.chars().count() {
            return None;
        }
        let lefts: Vec<f32> = glyphs.iter().map(|glyph| glyph.pos.x).collect();
        let rights: Vec<f32> = glyphs
            .iter()
            .map(|glyph| glyph.pos.x + glyph.advance_width)
            .collect();
        let end = rights.last().copied().unwrap_or(0.0);
        Some(Ruler { lefts, rights, end })
    }

    /// Lays out every part of the text alone, and spreads the characters of each part evenly
    /// over its width.
    // SMELL: one word in a script that runs right to left makes every word of the block
    // measured this way. A word that is wider than its row is then cut at a place that is only
    // about right, and its pieces can run a little past the width.
    fn by_part(ui: &Ui, parts: &[(String, TextFormat)]) -> Ruler {
        let (mut lefts, mut rights, mut x) = (Vec::new(), Vec::new(), 0.0);
        for (text, format) in parts {
            let galley =
                ui.painter()
                    .layout_no_wrap(text.clone(), format.font_id.clone(), Color32::WHITE);
            let width = galley.size().x;
            let chars = text.chars().count();
            for index in 0..chars {
                lefts.push(x + width * index as f32 / chars as f32);
                rights.push(x + width * (index + 1) as f32 / chars as f32);
            }
            x += width;
        }
        Ruler {
            lefts,
            rights,
            end: x,
        }
    }

    fn left(&self, glyph: usize) -> f32 {
        self.lefts.get(glyph).copied().unwrap_or(self.end)
    }

    fn right(&self, glyph: usize) -> f32 {
        self.rights.get(glyph).copied().unwrap_or(self.end)
    }

    /// The width of a piece and the room after it. Pieces that touch add up to the distance
    /// between their starts, so a line is as long as the text was laid out.
    fn measure(&self, slot: &Slot) -> (f32, f32) {
        let left = self.left(slot.start);
        let last = (slot.start + slot.chars).saturating_sub(1);
        let extent = (self.right(last) - left).max(0.0);
        let advance = self.left(slot.after) - left;
        match (slot.is_spaced, slot.is_joined) {
            (true, _) => (extent, (advance - extent).max(0.0)),
            (false, true) => (advance.max(extent), 0.0),
            (false, false) => (extent, 0.0),
        }
    }

    fn edges(&self, slot: &Slot, width: f32) -> Vec<f32> {
        let left = self.left(slot.start);
        let mut edges: Vec<f32> = (0..slot.chars)
            .map(|index| (self.left(slot.start + index) - left).clamp(0.0, width))
            .collect();
        edges.push(width);
        edges
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::Offload;
    use crate::media::rich_text::parse::parse;

    /// Runs `check` inside one pass of a window, with a manual formula cache.
    fn in_a_pass(check: impl FnOnce(&Ui, &mut Math)) {
        let ctx = egui::Context::default();
        // The fonts of the theme can be laid out only after the theme is installed.
        crate::theme::install(&ctx);
        let mut math = Math::new(&ctx, Offload::Manual);
        let mut check = Some(check);
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            if let Some(check) = check.take() {
                check(ui, &mut math);
            }
        });
        output.textures_delta.clear();
    }

    #[test]
    fn a_text_in_a_script_that_runs_right_to_left_is_still_measured() {
        in_a_pass(|ui, math| {
            let look = text_look(TextRole::Body);
            let text = "emoji 😀 e\u{301} ﬁ 日本語 مرحبا a\u{200d}b \u{feff}x \u{1f468}\u{200d}\u{1f469} tab\u{0}nul";
            let measured = measure(ui, &parse(text), &[], &look, math, WaitFor::OwnFormulas);
            let atoms = &measured.lines[0].atoms;
            assert!(atoms.iter().all(|atom| atom.width >= 0.0));
            let arabic = atoms
                .iter()
                .find(|atom| matches!(&atom.kind, AtomKind::Word { text, .. } if text == "مرحبا"));
            assert!(arabic.is_some_and(|atom| atom.width > 10.0 && atom.gap > 0.0));
        });
    }
}
