//! Draws a Markdown table: sizes each column to what is in it, lays out every cell, and scrolls
//! the table sideways when the columns do not fit.

use eframe::egui::{
    Painter, Pos2, Rect, ScrollArea, Sense, Ui, Vec2, WidgetInfo, WidgetType, vec2,
};

use super::Clicked;
use super::atom::{Measured, Seen};
use super::flow::{Flow, break_lines};
use super::measure::{WaitFor, measure};
use super::paint;
use super::parse::{Parsed, Span};
use super::parse_table::{ColumnAlign, ParsedTable};
use super::style::{TableLook, TextLook, header_look, rule, table_look, text_look};
use crate::media::math::{Math, MathRef, MathState};
use crate::theme::TextRole;

/// No column is made wider than this, however long the text in it is.
pub(super) const MAX_COLUMN_WIDTH: f32 = 360.0;

/// The table to lay out, and the room it has.
#[derive(Debug, Clone, Copy)]
pub(super) struct Source<'a> {
    pub(super) table: &'a ParsedTable,
    pub(super) role: TextRole,
    /// The hash of the stored text, which names the table among the widgets.
    pub(super) content: u64,
    /// The room the table has, in points.
    pub(super) width: f32,
}

/// A laid-out table. Every length is in points from its top left corner.
#[derive(Debug)]
pub(super) struct TableLayout {
    pub(super) content: u64,
    pub(super) size: Vec2,
    pub(super) rows: Vec<TableRow>,
    /// The columns are wider than the room, so the table scrolls sideways.
    pub(super) scrolls: bool,
    /// Some formula in some cell is loading: every cell holds the room of its formulas.
    pub(super) is_waiting: bool,
    /// The name of the table: its cells with ` | ` between them, a row on each line.
    pub(super) plain: String,
}

#[derive(Debug)]
pub(super) struct TableRow {
    pub(super) top: f32,
    pub(super) height: f32,
    pub(super) is_header: bool,
    pub(super) cells: Vec<TableCell>,
}

impl TableRow {
    /// Whether any part of the row is inside `view`, which is in the coordinates of the table.
    fn is_seen(&self, view: Rect) -> bool {
        self.top <= view.bottom() && self.top + self.height >= view.top()
    }

    fn plain(&self) -> String {
        let cells: Vec<&str> = self
            .cells
            .iter()
            .map(|cell| cell.flow.plain.as_str())
            .collect();
        cells.join(" | ")
    }
}

#[derive(Debug)]
pub(super) struct TableCell {
    pub(super) flow: Flow,
    /// Where the cell's text starts, from the corner of the table.
    pub(super) offset: Vec2,
}

/// The widths of the columns, and whether the table has to scroll. `mins` holds the least width
/// of each column and `maxes` the width that each would like. With room for every `max`, each
/// column has it. With less, each has its `min` and a share of the room that is left, in
/// proportion to how much it can still grow. With no room for every `min`, each has its `min`
/// and the table scrolls.
pub(super) fn column_widths(mins: &[f32], maxes: &[f32], available: f32) -> (Vec<f32>, bool) {
    let total_min: f32 = mins.iter().sum();
    let total_max: f32 = maxes.iter().sum();
    if total_max <= available {
        return (maxes.to_vec(), false);
    }
    if total_min <= available {
        let share = (available - total_min) / (total_max - total_min);
        let widths = mins
            .iter()
            .zip(maxes)
            .map(|(min, max)| min + (max - min) * share)
            .collect();
        return (widths, false);
    }
    (mins.to_vec(), true)
}

/// The cells of one row with their widths, before the columns have theirs.
struct MeasuredRow {
    cells: Vec<Measured>,
    is_header: bool,
}

/// For each column, the widest piece that cannot be broken and the widest cell in one line.
fn column_bounds(rows: &[MeasuredRow], columns: usize) -> (Vec<f32>, Vec<f32>) {
    let (mut mins, mut maxes) = (vec![0.0f32; columns], vec![0.0f32; columns]);
    for row in rows {
        for (column, cell) in row.cells.iter().enumerate() {
            maxes[column] = maxes[column].max(cell.widest_line().min(MAX_COLUMN_WIDTH));
            mins[column] = mins[column].max(cell.widest_unit());
        }
    }
    for (min, max) in mins.iter_mut().zip(&maxes) {
        *min = min.min(*max);
    }
    (mins, maxes)
}

/// Where each column starts, from the left edge of the table. `padding` is the room that a
/// column takes beside its text.
fn column_lefts(widths: &[f32], padding: f32) -> Vec<f32> {
    widths
        .iter()
        .scan(0.0, |left, width| {
            let at = *left;
            *left += width + padding;
            Some(at)
        })
        .collect()
}

/// Whether a formula in any cell is still loading. Every formula is asked for, with no early
/// exit, because the formula cache drops a loading formula that nobody asked for in a frame.
fn is_any_formula_loading(
    table: &ParsedTable,
    header: &TextLook,
    body: &TextLook,
    math: &mut Math,
) -> bool {
    let header_cells = table.header.iter().map(|cell| (cell, header.role));
    let body_cells = table.rows.iter().flatten().map(|cell| (cell, body.role));
    let mut loading = false;
    for (cell, role) in header_cells.chain(body_cells) {
        for span in cell.lines.iter().flat_map(|line| &line.spans) {
            if let Span::Math(latex) = span {
                loading |= math.get(&MathRef::inline(latex, role)) == MathState::Loading;
            }
        }
    }
    loading
}

/// What every row of a table shares: where its columns are, and what its cells look like.
struct Grid<'a> {
    widths: &'a [f32],
    lefts: &'a [f32],
    aligns: &'a [ColumnAlign],
    look: TableLook,
    header: &'a TextLook,
    body: &'a TextLook,
}

impl Grid<'_> {
    /// One row at `top`: every cell broken at the width of its column, and the row as high as
    /// its tallest cell.
    fn row(&self, measured: &MeasuredRow, top: f32) -> TableRow {
        let cell_look = if measured.is_header {
            self.header
        } else {
            self.body
        };
        let flows: Vec<Flow> = measured
            .cells
            .iter()
            .zip(self.widths)
            .map(|(cell, width)| break_lines(cell, *width, cell_look))
            .collect();
        let tallest = flows
            .iter()
            .map(|flow| flow.size.y)
            .fold(cell_look.line_height, f32::max);
        let cells = flows
            .into_iter()
            .enumerate()
            .map(|(column, flow)| {
                // SMELL: a cell that wraps is moved as one block, so its shorter rows are not
                // centred or set to the right one by one.
                let spare = (self.widths[column] - flow.size.x).max(0.0);
                let shift = match self.aligns[column] {
                    ColumnAlign::Left => 0.0,
                    ColumnAlign::Center => spare / 2.0,
                    ColumnAlign::Right => spare,
                };
                let offset = vec2(
                    self.lefts[column] + self.look.pad_x + shift,
                    top + self.look.pad_y,
                );
                TableCell { flow, offset }
            })
            .collect();
        TableRow {
            top,
            height: tallest + 2.0 * self.look.pad_y,
            is_header: measured.is_header,
            cells,
        }
    }
}

pub(super) fn build(ui: &Ui, math: &mut Math, source: &Source<'_>) -> TableLayout {
    let Source {
        table,
        role,
        content,
        width,
    } = *source;
    let (body, header) = (text_look(role), header_look());
    let is_waiting = is_any_formula_loading(table, &header, &body, math);
    let wait = if is_waiting {
        WaitFor::OthersToo
    } else {
        WaitFor::OwnFormulas
    };
    let mut measure_cells = |cells: &[Parsed], cell_look: &TextLook| -> Vec<Measured> {
        cells
            .iter()
            .map(|cell| measure(ui, cell, &[], cell_look, math, wait))
            .collect()
    };
    let mut measured = vec![MeasuredRow {
        cells: measure_cells(&table.header, &header),
        is_header: true,
    }];
    for row in &table.rows {
        measured.push(MeasuredRow {
            cells: measure_cells(row, &body),
            is_header: false,
        });
    }

    let look = table_look();
    let padding = 2.0 * look.pad_x;
    let (mins, maxes) = column_bounds(&measured, table.aligns.len());
    let available = (width - mins.len() as f32 * padding).max(0.0);
    let (widths, scrolls) = column_widths(&mins, &maxes, available);
    let lefts = column_lefts(&widths, padding);
    let grid = Grid {
        widths: &widths,
        lefts: &lefts,
        aligns: &table.aligns,
        look,
        header: &header,
        body: &body,
    };

    let mut rows: Vec<TableRow> = Vec::new();
    let mut top = 0.0;
    for measured_row in &measured {
        let row = grid.row(measured_row, top);
        top += row.height;
        rows.push(row);
    }
    let names: Vec<String> = rows.iter().map(TableRow::plain).collect();
    let right = lefts
        .last()
        .zip(widths.last())
        .map_or(0.0, |(left, width)| left + width + padding);
    TableLayout {
        content,
        size: vec2(right, top),
        rows,
        scrolls,
        is_waiting,
        plain: names.join("\n"),
    }
}

/// Whether the layout no longer matches the formulas that it shows, as for a block of text. A
/// table that waits is made again when no formula in any cell is loading any more.
pub(super) fn is_stale(layout: &TableLayout, math: &mut Math, view: Rect) -> bool {
    if !Rect::from_min_size(Pos2::ZERO, layout.size).intersects(view) {
        return false;
    }
    let mut stale = false;
    let mut loading = false;
    for row in &layout.rows {
        let row_is_seen = row.is_seen(view);
        for cell in &row.cells {
            if layout.is_waiting {
                for formula in cell.flow.formulas.iter().filter(|f| f.seen != Seen::Failed) {
                    loading |= math.get(&formula.math_ref()) == MathState::Loading;
                }
            } else if row_is_seen {
                let in_cell = view.translate(-cell.offset);
                stale |= paint::is_stale(&cell.flow, math, in_cell);
            }
        }
    }
    if layout.is_waiting { !loading } else { stale }
}

/// Draws the table and gives it one node, named by its cells. Returns `CopyText` with the
/// stored Markdown when the copy menu was used.
pub(super) fn show(
    ui: &mut Ui,
    math: &mut Math,
    layout: &TableLayout,
    markdown: &str,
) -> Option<Clicked> {
    // SMELL: the table and its scroll area are named by what the table holds, so that a table
    // keeps its scroll place when things above it come and go. Two equal tables in one parent
    // then share a name: they scroll together, and a debug build reports the clash on screen.
    let node = ui.id().with(("table", layout.content));
    let response = if layout.scrolls {
        // The node is as wide as the room, not as the table, so that it stays inside the panel.
        let outer = Rect::from_min_size(
            ui.next_widget_position(),
            vec2(ui.available_width(), layout.size.y),
        );
        let response = ui.interact(outer, node, Sense::click());
        ScrollArea::horizontal()
            .id_salt(layout.content)
            .show(ui, |ui| {
                let (rect, _) = ui.allocate_exact_size(layout.size, Sense::hover());
                draw(ui.painter(), layout, rect.min, math);
            });
        response
    } else {
        let (rect, response) = ui.allocate_exact_size(layout.size, Sense::click());
        if ui.is_rect_visible(rect) {
            draw(ui.painter(), layout, rect.min, math);
        }
        response
    };
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &layout.plain));
    paint::copy_is_chosen(&response, "Copy table").then(|| Clicked::CopyText(markdown.to_owned()))
}

fn draw(painter: &Painter, layout: &TableLayout, origin: Pos2, math: &mut Math) {
    let look = table_look();
    let view = painter.clip_rect().translate(-origin.to_vec2());
    for (index, row) in layout.rows.iter().enumerate() {
        if !row.is_seen(view) {
            continue;
        }
        let rect =
            Rect::from_min_size(origin + vec2(0.0, row.top), vec2(layout.size.x, row.height));
        if row.is_header {
            painter.rect_filled(rect, 0.0, look.header_fill);
        } else if index > 0 {
            painter.line_segment([rect.left_top(), rect.right_top()], rule(painter));
        }
        for cell in &row.cells {
            paint::draw(painter, &cell.flow, origin + cell.offset, math);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_take_their_widest_cell_then_share_what_is_left_then_scroll() {
        let (mins, maxes) = ([20.0, 40.0], [100.0, 200.0]);
        assert_eq!(
            column_widths(&mins, &maxes, 400.0),
            (vec![100.0, 200.0], false)
        );

        let (widths, scrolls) = column_widths(&mins, &maxes, 180.0);
        assert!(!scrolls);
        assert!((widths.iter().sum::<f32>() - 180.0).abs() < 0.01);
        assert!(widths[0] > 20.0 && widths[0] < 100.0 && widths[1] > 40.0 && widths[1] < 200.0);

        assert_eq!(column_widths(&mins, &maxes, 50.0), (vec![20.0, 40.0], true));
    }
}
