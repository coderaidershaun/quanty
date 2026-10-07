//! Turns the fragments of a row into what is drawn: runs of text, the fill behind code, formulas
//! and chips.

use eframe::egui::{Color32, FontId, Rect, pos2, text::LayoutJob, text::TextFormat, vec2};

use super::atom::{AtomKind, LineMetrics};
use super::breaker::{Fragment, text_between};
use super::measure::one_row_job;
use super::style::TextLook;

/// A word this close to where the text itself would put it counts as being there.
const ALIGN_TOLERANCE: f32 = 0.01;

/// The box of one row of a block.
#[derive(Debug, Clone, Copy)]
pub(super) struct RowBox {
    pub(super) top: f32,
    pub(super) height: f32,
    /// The baseline that the words, the formulas and the marks of the row stand on.
    pub(super) baseline: f32,
}

/// Words that stand next to each other in one font, to be laid out as one text. Words in
/// another font start a run of their own, because egui lines up two fonts in one text by the
/// bottom of their boxes, not by their baselines.
#[derive(Debug, Clone)]
pub(super) struct Run {
    pub(super) row: usize,
    pub(super) x: f32,
    /// How far the run is lifted above the baseline.
    pub(super) raise: f32,
    pub(super) job: LayoutJob,
}

/// The fill behind a code span.
#[derive(Debug, Clone, Copy)]
pub(super) struct CodeFill {
    pub(super) row: usize,
    pub(super) rect: Rect,
    pub(super) color: Color32,
    pub(super) radius: f32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PlacedMath {
    pub(super) formula: usize,
    pub(super) row: usize,
    /// The left end of the baseline of the formula.
    pub(super) x: f32,
    pub(super) scale: f32,
    pub(super) tint: Color32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PlacedChip {
    pub(super) number: usize,
    pub(super) rect: Rect,
}

/// One row to place: its number in the block, its box, and what its text looks like.
pub(super) struct Row<'a> {
    pub(super) index: usize,
    pub(super) bounds: RowBox,
    pub(super) metrics: &'a LineMetrics,
    pub(super) look: &'a TextLook,
}

/// What is drawn of a block, added row after row.
#[derive(Debug, Clone, Default)]
pub(super) struct Placed {
    pub(super) runs: Vec<Run>,
    pub(super) code_fills: Vec<CodeFill>,
    pub(super) maths: Vec<PlacedMath>,
    pub(super) chips: Vec<PlacedChip>,
}

impl Placed {
    pub(super) fn add_row(&mut self, fragments: &[Fragment<'_>], row: &Row<'_>) {
        self.add_code_fills(fragments, row);
        let mut run: Option<RunBuilder> = None;
        for fragment in fragments {
            match &fragment.atom.kind {
                AtomKind::Word { text, format, .. } => {
                    let word = text_between(text, fragment.from, fragment.to);
                    match &mut run {
                        Some(open) if open.accepts(format, fragment) => {
                            open.push(word, format, fragment);
                        }
                        _ => {
                            self.finish_run(run.take(), row.index);
                            run = Some(RunBuilder::start(word, format, fragment));
                        }
                    }
                }
                AtomKind::Foot { text, format } => {
                    self.finish_run(run.take(), row.index);
                    let mut mark = RunBuilder::start(text.clone(), format, fragment);
                    mark.raise = row.metrics.foot_raise;
                    self.finish_run(Some(mark), row.index);
                }
                AtomKind::Math {
                    formula,
                    is_painted,
                } => {
                    self.finish_run(run.take(), row.index);
                    if *is_painted {
                        self.maths.push(PlacedMath {
                            formula: *formula,
                            row: row.index,
                            x: fragment.x,
                            scale: fragment.scale,
                            tint: row.metrics.tint,
                        });
                    }
                }
                AtomKind::Chip { number } => {
                    self.finish_run(run.take(), row.index);
                    let atom = fragment.atom;
                    let size = vec2(atom.width, atom.ascent + atom.descent);
                    let y = row.bounds.top + (row.bounds.height - size.y) / 2.0;
                    self.chips.push(PlacedChip {
                        number: *number,
                        rect: Rect::from_min_size(pos2(fragment.x, y), size),
                    });
                }
            }
        }
        self.finish_run(run.take(), row.index);
    }

    /// One fill behind each run of code words that stand next to each other.
    fn add_code_fills(&mut self, fragments: &[Fragment<'_>], row: &Row<'_>) {
        let mut open: Option<(f32, f32)> = None;
        for fragment in fragments {
            if fragment.atom.is_code() {
                let left = open.map_or(fragment.x, |(left, _)| left);
                open = Some((left, fragment.x + fragment.width));
            } else if let Some((left, right)) = open.take() {
                self.add_code_fill(left, right, row);
            }
        }
        if let Some((left, right)) = open {
            self.add_code_fill(left, right, row);
        }
    }

    fn add_code_fill(&mut self, left: f32, right: f32, row: &Row<'_>) {
        let pad = row.look.code_pad;
        let top = row.bounds.baseline - row.metrics.code_ascent - pad;
        let bottom = row.bounds.baseline + row.metrics.code_descent + pad;
        self.code_fills.push(CodeFill {
            row: row.index,
            rect: Rect::from_min_max(pos2(left - pad, top), pos2(right + pad, bottom)),
            color: row.look.code_fill,
            radius: row.look.code_radius,
        });
    }

    fn finish_run(&mut self, run: Option<RunBuilder>, row: usize) {
        if let Some(run) = run {
            self.runs.push(run.finish(row));
        }
    }
}

/// The words of a run so far.
struct RunBuilder {
    x: f32,
    font: FontId,
    raise: f32,
    parts: Vec<(String, TextFormat)>,
    /// The room after the last word, which is a space if the next word joins the run.
    gap: f32,
    /// Where the next word starts if it follows the last one at once.
    next_x: f32,
}

impl RunBuilder {
    fn start(text: String, format: &TextFormat, fragment: &Fragment<'_>) -> RunBuilder {
        RunBuilder {
            x: fragment.x,
            font: format.font_id.clone(),
            raise: 0.0,
            parts: vec![(text, format.clone())],
            gap: fragment.atom.gap,
            next_x: fragment.x + fragment.width + fragment.atom.gap,
        }
    }

    /// Whether the word can be set after the words of the run: the same font, and a place
    /// where the text itself would put it. A list item hangs, so its text starts later.
    fn accepts(&self, format: &TextFormat, fragment: &Fragment<'_>) -> bool {
        self.font == format.font_id && (fragment.x - self.next_x).abs() < ALIGN_TOLERANCE
    }

    fn push(&mut self, text: String, format: &TextFormat, fragment: &Fragment<'_>) {
        // The space goes to the end of the word before it, in the format of that word, as it
        // was when the words were measured.
        if self.gap > 0.0
            && let Some((last, _)) = self.parts.last_mut()
        {
            last.push(' ');
        }
        self.parts.push((text, format.clone()));
        self.gap = fragment.atom.gap;
        self.next_x = fragment.x + fragment.width + fragment.atom.gap;
    }

    fn finish(self, row: usize) -> Run {
        Run {
            row,
            x: self.x,
            raise: self.raise,
            job: one_row_job(&self.parts),
        }
    }
}
