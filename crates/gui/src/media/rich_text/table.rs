//! Draws a Markdown table: sizes each column to what is in it, lays out every cell, and scrolls
//! the table sideways when the columns do not fit.

use eframe::egui::{
    Align, Painter, Pos2, Rect, ScrollArea, Sense, Ui, Vec2, WidgetInfo, WidgetType, vec2,
};

use super::Clicked;
use super::atom::{Measured, Seen};
use super::flow::{Flow, break_lines};
use super::measure::{WaitFor, alone_width, measure};
use super::paint;
use super::parse::Span;
use super::parse_table::{ColumnAlign, ParsedTable};
use super::style::{TableLook, TextLook, header_look, rule, table_look, text_look};
use crate::media::content_salt;
use crate::media::math::{Math, MathRef, MathState};
use crate::theme::TextRole;

pub(super) const MAX_COLUMN_WIDTH: f32 = 360.0;

#[derive(Debug, Clone, Copy)]
pub(super) struct Source<'a> {
    pub(super) table: &'a ParsedTable,
    pub(super) role: TextRole,
    /// The hash of the stored text, which names the table among the widgets.
    pub(super) content: u64,
    /// The room the table has, in points.
    pub(super) width: f32,
}

/// Every length is in points from its top left corner.
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

/// Returns the widths of the columns, and whether the table has to scroll. With too little room
/// for every `max`, each column has its `min` and a share of the room that is left, in
/// proportion to how much it can still grow.
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

/// `padding` is the room that a column takes beside its text.
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

struct RowLooks {
    header: TextLook,
    body: TextLook,
}

impl RowLooks {
    fn of(&self, is_header: bool) -> &TextLook {
        if is_header { &self.header } else { &self.body }
    }
}

/// Every formula is asked for, with no early exit, because the formula cache drops a loading
/// formula that nobody asked for in a frame.
fn is_any_formula_loading(table: &ParsedTable, looks: &RowLooks, math: &mut Math) -> bool {
    let header_cells = table.header.iter().map(|cell| (cell, looks.header.role));
    let body_cells = table
        .rows
        .iter()
        .flatten()
        .map(|cell| (cell, looks.body.role));
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

/// While a formula of any cell is loading, every cell waits, so that the cells show their
/// formulas together.
fn measure_rows(
    ui: &Ui,
    math: &mut Math,
    table: &ParsedTable,
    looks: &RowLooks,
) -> (Vec<MeasuredRow>, bool) {
    let is_waiting = is_any_formula_loading(table, looks, math);
    let wait = if is_waiting {
        WaitFor::OthersToo
    } else {
        WaitFor::OwnFormulas
    };
    let header = std::iter::once((&table.header, true));
    let body = table.rows.iter().map(|row| (row, false));
    let rows = header
        .chain(body)
        .map(|(cells, is_header)| MeasuredRow {
            cells: cells
                .iter()
                .map(|cell| measure(ui, cell, &[], looks.of(is_header), math, wait))
                .collect(),
            is_header,
        })
        .collect();
    (rows, is_waiting)
}

fn row_align(align: ColumnAlign) -> Align {
    match align {
        ColumnAlign::Left => Align::Min,
        ColumnAlign::Center => Align::Center,
        ColumnAlign::Right => Align::Max,
    }
}

struct Grid<'a> {
    ui: &'a Ui,
    widths: &'a [f32],
    lefts: &'a [f32],
    aligns: &'a [ColumnAlign],
    look: TableLook,
    looks: &'a RowLooks,
}

impl Grid<'_> {
    /// Every row, each under the one before it, and how tall they are together.
    fn rows(&self, measured: &[MeasuredRow]) -> (Vec<TableRow>, f32) {
        let mut rows: Vec<TableRow> = Vec::new();
        let mut top = 0.0;
        for measured_row in measured {
            let row = self.row(measured_row, top);
            top += row.height;
            rows.push(row);
        }
        (rows, top)
    }

    fn row(&self, measured: &MeasuredRow, top: f32) -> TableRow {
        let cell_look = self.looks.of(measured.is_header);
        let piece_width = alone_width(self.ui);
        let flows: Vec<Flow> = measured
            .cells
            .iter()
            .zip(self.widths)
            .zip(self.aligns)
            .map(|((cell, width), align)| {
                let look = TextLook {
                    align: row_align(*align),
                    ..cell_look.clone()
                };
                break_lines(cell, *width, &look, &piece_width)
            })
            .collect();
        let tallest = flows
            .iter()
            .map(|flow| flow.size.y)
            .fold(cell_look.line_height, f32::max);
        let cells = flows
            .into_iter()
            .zip(self.lefts)
            .map(|(flow, left)| {
                let offset = vec2(left + self.look.pad_x, top + self.look.pad_y);
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
    let looks = RowLooks {
        header: header_look(),
        body: text_look(role),
    };
    let (measured, is_waiting) = measure_rows(ui, math, table, &looks);

    let look = table_look();
    let padding = 2.0 * look.pad_x;
    let (mins, maxes) = column_bounds(&measured, table.aligns.len());
    let available = (width - mins.len() as f32 * padding).max(0.0);
    let (widths, scrolls) = column_widths(&mins, &maxes, available);
    let lefts = column_lefts(&widths, padding);
    let grid = Grid {
        ui,
        widths: &widths,
        lefts: &lefts,
        aligns: &table.aligns,
        look,
        looks: &looks,
    };
    let (rows, top) = grid.rows(&measured);
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

pub(super) fn show(
    ui: &mut Ui,
    math: &mut Math,
    layout: &TableLayout,
    markdown: &str,
) -> Option<Clicked> {
    let salt = content_salt(ui, layout.content);
    let node = ui.id().with(("table", salt));
    let response = if layout.scrolls {
        // The node is as wide as the room, not as the table, so that it stays inside the panel.
        let outer = Rect::from_min_size(
            ui.next_widget_position(),
            vec2(ui.available_width(), layout.size.y),
        );
        let response = ui.interact(outer, node, Sense::click());
        ScrollArea::horizontal().id_salt(salt).show(ui, |ui| {
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
