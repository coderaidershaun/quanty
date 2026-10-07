//! Reads a GitHub Markdown table: a header line, a line of dashes that says how each column is
//! aligned, and the rows below.

use super::parse::{Parsed, clean, parse_cell};

/// How the text of one column sits in its cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ColumnAlign {
    Left,
    Center,
    Right,
}

/// A table that was read. The header and every row have one cell for each column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParsedTable {
    pub(super) aligns: Vec<ColumnAlign>,
    pub(super) header: Vec<Parsed>,
    pub(super) rows: Vec<Vec<Parsed>>,
}

/// Reads a table. `None` says that the text is not one: it has no bar in its first line, or no
/// line of dashes with as many cells under it.
pub(super) fn parse_table(markdown: &str) -> Option<ParsedTable> {
    let cleaned = clean(markdown);
    let mut lines = cleaned
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty());
    let header_line = lines.next()?;
    let header = split_cells(header_line);
    let aligns: Vec<ColumnAlign> = split_cells(lines.next()?)
        .iter()
        .map(|cell| align_of(cell))
        .collect::<Option<_>>()?;
    if !header_line.contains('|') || header.is_empty() || aligns.len() != header.len() {
        return None;
    }
    let columns = header.len();
    // Every row gets as many cells as the header has, because the layout finds a cell by the
    // number of its column: a short row is filled up, and a long one loses its last cells.
    let cells = |texts: Vec<String>| -> Vec<Parsed> {
        (0..columns)
            .map(|column| parse_cell(texts.get(column).map_or("", String::as_str)))
            .collect()
    };
    Some(ParsedTable {
        aligns,
        header: cells(header),
        rows: lines.map(|line| cells(split_cells(line))).collect(),
    })
}

/// The cells of one line of a table, with no cell for the bars at its two ends.
fn split_cells(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                cell.push('|');
                chars.next();
            }
            '|' => cells.push(std::mem::take(&mut cell)),
            other => cell.push(other),
        }
    }
    cells.push(cell);
    if line.starts_with('|') {
        cells.remove(0);
    }
    if line.ends_with('|') && !line.ends_with("\\|") {
        cells.pop();
    }
    cells.iter().map(|cell| cell.trim().to_owned()).collect()
}

/// The alignment that a cell of the dashed line asks for, or `None` if it is not such a cell.
fn align_of(cell: &str) -> Option<ColumnAlign> {
    let dashes = cell.trim_matches(':');
    if dashes.is_empty() || !dashes.chars().all(|c| c == '-') {
        return None;
    }
    Some(match (cell.starts_with(':'), cell.ends_with(':')) {
        (true, true) => ColumnAlign::Center,
        (false, true) => ColumnAlign::Right,
        _ => ColumnAlign::Left,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_table_has_a_header_alignments_and_padded_rows() {
        let table =
            parse_table("| | a *b* |c \\| d|\n|:--|:-:|--:|\n| x | y |\n| 1 | 2 | 3 | 4 |\n")
                .expect("a table");
        assert_eq!(
            table.aligns,
            [ColumnAlign::Left, ColumnAlign::Center, ColumnAlign::Right]
        );
        let plain = |cells: &[Parsed]| {
            cells
                .iter()
                .map(|cell| cell.plain.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(plain(&table.header), ["", "a b", "c | d"]);
        assert_eq!(plain(&table.rows[0]), ["x", "y", ""]);
        assert_eq!(plain(&table.rows[1]), ["1", "2", "3"]);
    }
}
