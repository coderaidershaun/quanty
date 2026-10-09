//! Breaks measured lines into rows that fit a width, and puts every word, formula and chip in
//! its place on its row. It works on numbers only and never touches the window.

use eframe::egui::{Align, Rect, Vec2, vec2};

use super::atom::{AtomKind, Formula, LineMetrics, Measured};
use super::breaker::{Fragment, PieceWidth, break_line};
use super::place::{CodeFill, Placed, PlacedChip, PlacedMath, Row, RowBox, Run};
use super::style::TextLook;

/// A block of text, broken into rows. Every length is in points from the top left corner of
/// the block.
///
/// Do not keep laid-out text here, only the jobs that lay it out. egui throws its glyphs away
/// when its store of them is nearly full, and text that was laid out before that draws wrong
/// glyphs afterwards.
#[derive(Debug, Clone)]
pub(super) struct Flow {
    pub(super) size: Vec2,
    pub(super) rows: Vec<RowBox>,
    pub(super) runs: Vec<Run>,
    pub(super) code_fills: Vec<CodeFill>,
    pub(super) maths: Vec<PlacedMath>,
    pub(super) chips: Vec<PlacedChip>,
    pub(super) formulas: Vec<Formula>,
    /// Some formula was loading when the block was laid out.
    pub(super) is_waiting: bool,
    pub(super) plain: String,
}

impl Flow {
    /// Whether any part of the row is inside `view`, which is in the same coordinates.
    pub(super) fn is_seen(&self, row: usize, view: Rect) -> bool {
        let row = self.rows[row];
        row.top <= view.bottom() && row.top + row.height >= view.top()
    }
}

pub(super) fn break_lines(
    measured: &Measured,
    wrap: f32,
    look: &TextLook,
    piece_width: PieceWidth<'_>,
) -> Flow {
    let mut rows: Vec<RowBox> = Vec::new();
    let mut placed = Placed::default();
    let mut y = 0.0;
    let mut width: f32 = 0.0;
    for line in &measured.lines {
        if line.starts_paragraph && !rows.is_empty() {
            y += look.paragraph_gap;
        }
        let mut rows_of_line = break_line(line, wrap, piece_width);
        if let Some(marker) = &line.marker {
            rows_of_line[0].insert(0, Fragment::whole(marker, 0.0));
        }
        for fragments in &mut rows_of_line {
            align_row(fragments, wrap, look.align);
            width = fragments.iter().fold(width, |wide, fragment| {
                wide.max(fragment.x + fragment.width)
            });
            let row = Row {
                index: rows.len(),
                bounds: row_box(fragments, &line.metrics, y, measured.pixels_per_point),
                metrics: &line.metrics,
                look,
            };
            placed.add_row(fragments, &row);
            rows.push(row.bounds);
            y = row.bounds.top + row.bounds.height;
        }
    }
    let Placed {
        runs,
        code_fills,
        maths,
        chips,
    } = placed;
    Flow {
        size: vec2(width, y),
        rows,
        runs,
        code_fills,
        maths,
        chips,
        formulas: measured.formulas.clone(),
        is_waiting: measured.is_waiting,
        plain: measured.plain.clone(),
    }
}

/// A row that is wider than its room stays at the left.
fn align_row(fragments: &mut [Fragment<'_>], wrap: f32, align: Align) {
    // The room of a block of text can be endless, and no shift can be worked out from that.
    if align == Align::Min {
        return;
    }
    let right = fragments.iter().fold(0.0, |right: f32, fragment| {
        right.max(fragment.x + fragment.width)
    });
    let shift = (wrap - right).max(0.0) * align.to_factor();
    for fragment in fragments {
        fragment.x += shift;
    }
}

fn round_to_pixel(value: f32, pixels_per_point: f32) -> f32 {
    (value * pixels_per_point).round() / pixels_per_point
}

fn row_box(
    fragments: &[Fragment<'_>],
    metrics: &LineMetrics,
    y: f32,
    pixels_per_point: f32,
) -> RowBox {
    let mut above = metrics.baseline;
    let mut below = metrics.line_height - metrics.baseline;
    let mut at_least: f32 = 0.0;
    for fragment in fragments {
        match fragment.atom.kind {
            AtomKind::Math { .. } => {
                above = above.max(fragment.atom.ascent * fragment.scale);
                below = below.max(fragment.atom.descent * fragment.scale);
            }
            AtomKind::Chip { .. } => {
                at_least = at_least.max(fragment.atom.ascent + fragment.atom.descent);
            }
            _ => {}
        }
    }
    let extra = ((at_least - above - below) / 2.0).max(0.0);
    above += extra;
    below += extra;
    let top = round_to_pixel(y, pixels_per_point);
    let baseline = round_to_pixel(top + above, pixels_per_point);
    let bottom = round_to_pixel(top + above + below, pixels_per_point).max(baseline);
    RowBox {
        top,
        height: bottom - top,
        baseline,
    }
}
