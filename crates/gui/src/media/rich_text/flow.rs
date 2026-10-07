//! Breaks measured lines into rows that fit a width, and puts every word, formula and chip in
//! its place on its row. It works on numbers only and never touches the window.

use eframe::egui::{Rect, Vec2, vec2};

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

/// Puts the atoms of a measured text in rows no wider than `wrap`, with the rows of one line
/// under each other and a gap between paragraphs.
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
        for fragments in &rows_of_line {
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

fn round_to_pixel(value: f32, pixels_per_point: f32) -> f32 {
    (value * pixels_per_point).round() / pixels_per_point
}

/// The box of the row that starts at `y`. It is as high as a line of text, and higher for a
/// formula that reaches above or below that, or for a chip that is taller. Its top, its
/// baseline and its bottom fall on whole pixels.
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

#[cfg(test)]
mod tests {
    use eframe::egui::{Color32, text::TextFormat};

    use super::*;
    use crate::media::rich_text::atom::{Atom, MeasuredLine, Seen};
    use crate::media::rich_text::parse::SpanStyle;
    use crate::media::rich_text::style::text_look;
    use crate::theme::TextRole;

    const GAP: f32 = 5.0;

    /// Ten points for each character, which is how wide the made-up words below are.
    fn ten_a_character(text: &str, _: &TextFormat) -> f32 {
        10.0 * text.chars().count() as f32
    }

    fn word(text: &str, width: f32, spaced: bool) -> Atom {
        let chars = text.chars().count();
        let edges = (0..=chars)
            .map(|i| width * i as f32 / chars as f32)
            .collect();
        Atom {
            kind: AtomKind::Word {
                text: text.to_owned(),
                style: SpanStyle::Plain,
                format: TextFormat::default(),
                edges,
            },
            width,
            ascent: 0.0,
            descent: 0.0,
            gap: if spaced { GAP } else { 0.0 },
            is_glued: !spaced,
        }
    }

    fn formula(width: f32, ascent: f32, descent: f32) -> Atom {
        Atom {
            kind: AtomKind::Math {
                formula: 0,
                is_painted: true,
            },
            width,
            ascent,
            descent,
            gap: GAP,
            is_glued: false,
        }
    }

    fn measured(atoms: Vec<Atom>) -> Measured {
        let metrics = LineMetrics {
            line_height: 22.0,
            baseline: 16.0,
            code_ascent: 10.0,
            code_descent: 3.0,
            foot_raise: 4.0,
            tint: Color32::WHITE,
        };
        let line = MeasuredLine {
            marker: None,
            atoms,
            indent: 0.0,
            starts_paragraph: false,
            metrics,
        };
        let seen = Seen::Ready {
            width: 20.0,
            ascent: 30.0,
            descent: 8.0,
        };
        Measured {
            lines: vec![line],
            formulas: vec![Formula {
                latex: "x".to_owned(),
                role: TextRole::Body,
                seen,
            }],
            is_waiting: false,
            plain: String::new(),
            pixels_per_point: 1.0,
        }
    }

    #[test]
    fn a_formula_sits_on_the_text_baseline_and_a_tall_one_grows_only_its_line() {
        let atoms = vec![
            word("aaaa", 40.0, true),
            word("bbbb", 40.0, true),
            word("cccc", 40.0, true),
            word("dddd", 40.0, true),
            formula(20.0, 30.0, 8.0),
            word("eeee", 40.0, true),
            word("ff", 60.0, false),
            word("gg", 60.0, true),
            word("hhhh", 40.0, true),
        ];
        let look = text_look(TextRole::Body);
        let flow = break_lines(&measured(atoms), 130.0, &look, &ten_a_character);

        let heights: Vec<f32> = flow.rows.iter().map(|row| row.height).collect();
        assert_eq!(
            heights,
            [22.0, 38.0, 22.0, 22.0],
            "only the row with the formula grows"
        );
        let rise: Vec<f32> = flow.rows.iter().map(|row| row.baseline - row.top).collect();
        assert_eq!(rise, [16.0, 30.0, 16.0, 16.0]);

        let [placed] = flow.maths[..] else {
            panic!("one formula is placed");
        };
        assert_eq!(placed.row, 1);
        assert_eq!(placed.x, 45.0, "it follows the word before it and a space");
        let beside: Vec<(f32, usize)> = flow
            .runs
            .iter()
            .filter(|run| run.row == 1)
            .map(|run| (run.x, run.row))
            .collect();
        assert_eq!(
            beside,
            [(0.0, 1), (70.0, 1)],
            "the words stand on the same row"
        );

        let texts: Vec<(&str, usize)> = flow
            .runs
            .iter()
            .map(|run| (run.job.text.as_str(), run.row))
            .collect();
        assert!(
            texts.contains(&("ffgg", 2)),
            "a unit with no space in it never splits: {texts:?}"
        );
        assert!(texts.contains(&("hhhh", 3)));
    }

    #[test]
    fn a_word_wider_than_its_row_is_cut_and_a_wide_formula_is_made_smaller() {
        let atoms = vec![word("abcdefghij", 100.0, true), formula(200.0, 10.0, 4.0)];
        let look = text_look(TextRole::Body);
        let flow = break_lines(&measured(atoms), 35.0, &look, &ten_a_character);
        let pieces: Vec<&str> = flow.runs.iter().map(|run| run.job.text.as_str()).collect();
        assert_eq!(pieces, ["abc", "def", "ghi", "j"]);
        let [placed] = flow.maths[..] else {
            panic!("one formula is placed");
        };
        assert!(placed.scale < 0.5);
        assert!(flow.size.x <= 35.5 + 1.0);
    }
}
