//! A table as chunks of a page: the header and each row are a chunk, so that a table can run over
//! a page between two rows but a row never splits.

use pulldown_cmark::Alignment;

use super::TEXT_WIDTH;
use super::flow::{Align, Atom, Layout, Line, Look, SMALL};
use super::pages::{Chunk, INK_RULE, Item};
use super::pieces::space;
use super::read::{Span, Table};

const PADDING_X: f32 = 4.0;
const PADDING_Y: f32 = 3.0;
const HEAVY_RULE: f32 = 1.0;
const LIGHT_RULE: f32 = 0.5;

impl Layout<'_> {
    pub(super) fn table(&mut self, table: &Table) -> Vec<Chunk> {
        let columns = table
            .rows
            .iter()
            .map(Vec::len)
            .chain([table.header.len()])
            .max()
            .unwrap_or(0);
        if columns == 0 {
            return Vec::new();
        }
        let head = SMALL.strong();
        let body = SMALL;
        let header = self.cells(&table.header, head, columns);
        let rows: Vec<Vec<Vec<Atom>>> = table
            .rows
            .iter()
            .map(|row| self.cells(row, body, columns))
            .collect();
        let widths = column_widths(columns, std::iter::once(&header).chain(rows.iter()));
        let total: f32 = widths.iter().sum();
        let left = (TEXT_WIDTH - total) / 2.0;

        let mut chunks = Vec::new();
        let sections =
            std::iter::once((&header, head, true)).chain(rows.iter().map(|row| (row, body, false)));
        let last = rows.len();
        for (at, (cells, look, is_header)) in sections.enumerate() {
            let mut laid: Vec<Vec<Line>> = Vec::new();
            for (cell, width) in cells.iter().zip(&widths) {
                laid.push(self.lines(cell.clone(), width - 2.0 * PADDING_X, look));
            }
            let height = laid
                .iter()
                .map(|lines| lines.iter().map(Line::height).sum::<f32>())
                .fold(0.0, f32::max)
                + 2.0 * PADDING_Y;
            let mut items = Vec::new();
            let mut x = left;
            for ((lines, width), column) in laid.iter().zip(&widths).zip(0..) {
                let align = match table.alignments.get(column) {
                    Some(Alignment::Center) => Align::Center,
                    Some(Alignment::Right) => Align::Right,
                    _ => Align::Left,
                };
                self.draw_lines(
                    lines,
                    (x + PADDING_X, PADDING_Y, width - 2.0 * PADDING_X),
                    align,
                    &mut items,
                );
                x += width;
            }
            let rule = |y: f32, thickness: f32| Item::Rule {
                x1: left,
                x2: left + total,
                y,
                thickness,
                ink: INK_RULE,
            };
            if is_header {
                items.push(rule(HEAVY_RULE / 2.0, HEAVY_RULE));
                items.push(rule(height - LIGHT_RULE / 2.0, LIGHT_RULE));
            }
            if at == last {
                items.push(rule(height - HEAVY_RULE / 2.0, HEAVY_RULE));
            }
            let mut chunk = Chunk::new(height, items);
            if is_header {
                chunk.keep_with_next = 1;
            }
            chunks.push(chunk);
        }
        space(&mut chunks, 4.0, 10.0);
        chunks
    }

    /// The atoms of each cell of a row, with an empty cell for each one the row lacks.
    fn cells(&mut self, row: &[Vec<Span>], look: Look, columns: usize) -> Vec<Vec<Atom>> {
        (0..columns)
            .map(|column| {
                row.get(column)
                    .map(|spans| self.atoms(spans, look))
                    .unwrap_or_default()
            })
            .collect()
    }
}

/// A column is as wide as its widest cell and its padding, when the table fits in the text
/// width. When it does not, each column keeps room for its longest word, the rest of the width
/// goes to the columns in proportion to how much more each one wants, and the cells wrap. Only
/// when the longest words alone are too wide does each column get a share in proportion to its
/// longest word, and words are cut.
fn column_widths<'a>(columns: usize, rows: impl Iterator<Item = &'a Vec<Vec<Atom>>>) -> Vec<f32> {
    let mut wanted = vec![0.0_f32; columns];
    let mut least = vec![0.0_f32; columns];
    for row in rows {
        for ((want, keep), cell) in wanted.iter_mut().zip(least.iter_mut()).zip(row) {
            let width: f32 = cell.iter().map(Atom::width).sum();
            *want = want.max(width + 2.0 * PADDING_X);
            *keep = keep.max(longest_word(cell) + 2.0 * PADDING_X);
        }
    }
    let wanted_sum: f32 = wanted.iter().sum();
    if wanted_sum <= TEXT_WIDTH {
        return wanted;
    }
    let least_sum: f32 = least.iter().sum();
    if least_sum >= TEXT_WIDTH {
        return least
            .iter()
            .map(|keep| keep / least_sum * TEXT_WIDTH)
            .collect();
    }
    let spare = TEXT_WIDTH - least_sum;
    let more_wanted = wanted_sum - least_sum;
    least
        .iter()
        .zip(&wanted)
        .map(|(keep, want)| keep + (want - keep) / more_wanted * spare)
        .collect()
}

/// The width of the widest run of atoms with no space in it: the narrowest a cell can be without
/// a word cut in two.
fn longest_word(cell: &[Atom]) -> f32 {
    cell.split(|atom| matches!(atom, Atom::Space { .. } | Atom::Break))
        .map(|word| word.iter().map(Atom::width).sum::<f32>())
        .fold(0.0, f32::max)
}
