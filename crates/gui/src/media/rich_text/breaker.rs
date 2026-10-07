//! Breaks the atoms of one line into rows: greedy, with no break inside a run of atoms that
//! have no space between them, and a cut for a word that is wider than a whole row.

use eframe::egui::text::TextFormat;

use super::atom::{Atom, AtomKind, MeasuredLine, unit_width};

/// A row may be this much longer than the width: words are measured in one line of text and
/// drawn in several, and the two differ by a fraction of a point.
const SLACK: f32 = 0.25;
/// A formula that is wider than its row is not drawn smaller than this share of its size.
const SMALLEST_SCALE: f32 = 0.05;

/// How wide a piece of a word is when it is laid out alone, given its text and its format.
pub(super) type PieceWidth<'a> = &'a dyn Fn(&str, &TextFormat) -> f32;

/// An atom, or a part of one, that stands on a row. Only a word that is wider than its row is
/// cut in parts.
pub(super) struct Fragment<'a> {
    pub(super) atom: &'a Atom,
    /// For a word: the characters that it holds, from `from` up to `to`.
    pub(super) from: usize,
    pub(super) to: usize,
    pub(super) x: f32,
    pub(super) width: f32,
    /// How much a formula is made smaller to fit its row.
    pub(super) scale: f32,
}

impl<'a> Fragment<'a> {
    pub(super) fn whole(atom: &'a Atom, x: f32) -> Fragment<'a> {
        let to = match &atom.kind {
            AtomKind::Word { edges, .. } => edges.len().saturating_sub(1),
            _ => 0,
        };
        Fragment {
            atom,
            from: 0,
            to,
            x,
            width: atom.width,
            scale: 1.0,
        }
    }
}

/// The characters of a word from `from` up to `to`.
pub(super) fn text_between(word: &str, from: usize, to: usize) -> String {
    word.chars().skip(from).take(to - from).collect()
}

/// The rows of one line.
pub(super) fn break_line<'a>(
    line: &'a MeasuredLine,
    wrap: f32,
    piece_width: PieceWidth<'a>,
) -> Vec<Vec<Fragment<'a>>> {
    let mut breaker = Breaker {
        rows: Vec::new(),
        row: Vec::new(),
        x: line.indent,
        indent: line.indent,
        wrap,
        piece_width,
    };
    for unit in line.units() {
        let width = unit_width(unit);
        if breaker.x + width > breaker.limit() && !breaker.row.is_empty() {
            breaker.next_row();
        }
        if breaker.x + width <= breaker.limit() {
            unit.iter().for_each(|atom| breaker.place_whole(atom));
        } else {
            unit.iter().for_each(|atom| breaker.place_alone(atom));
        }
    }
    breaker.finish()
}

struct Breaker<'a> {
    rows: Vec<Vec<Fragment<'a>>>,
    row: Vec<Fragment<'a>>,
    x: f32,
    indent: f32,
    wrap: f32,
    piece_width: PieceWidth<'a>,
}

impl<'a> Breaker<'a> {
    fn limit(&self) -> f32 {
        self.wrap + SLACK
    }

    fn next_row(&mut self) {
        self.rows.push(std::mem::take(&mut self.row));
        self.x = self.indent;
    }

    fn finish(mut self) -> Vec<Vec<Fragment<'a>>> {
        self.rows.push(self.row);
        self.rows
    }

    fn put(&mut self, fragment: Fragment<'a>) {
        self.x += fragment.width;
        self.row.push(fragment);
    }

    fn place_whole(&mut self, atom: &'a Atom) {
        self.put(Fragment::whole(atom, self.x));
        self.x += atom.gap;
    }

    /// For an atom of a unit that is wider than a whole row: the unit breaks between atoms,
    /// and an atom that is still too wide is cut or made smaller.
    fn place_alone(&mut self, atom: &'a Atom) {
        if self.x + atom.width > self.limit() && !self.row.is_empty() {
            self.next_row();
        }
        if self.x + atom.width <= self.limit() {
            return self.place_whole(atom);
        }
        match &atom.kind {
            AtomKind::Word {
                text,
                format,
                edges,
                ..
            } => {
                let piece_width = self.piece_width;
                self.cut(atom, edges, |from, to| {
                    piece_width(&text_between(text, from, to), format)
                });
            }
            AtomKind::Math { .. } => self.shrink(atom),
            AtomKind::Foot { .. } | AtomKind::Chip { .. } => self.place_whole(atom),
        }
    }

    /// Sets a formula that is wider than its row at the scale at which it fits.
    fn shrink(&mut self, atom: &'a Atom) {
        let room = (self.wrap - self.x).max(1.0);
        let scale = (room / atom.width).clamp(SMALLEST_SCALE, 1.0);
        let mut fragment = Fragment::whole(atom, self.x);
        fragment.scale = scale;
        fragment.width = atom.width * scale;
        self.put(fragment);
        self.x += atom.gap;
    }

    /// Cuts a word at the last character whose piece fits, as many times as it takes. A piece is
    /// measured alone, as it is drawn. In the one line of text, the kerning between the last
    /// character of a piece and the next one is already in the left edge of the next one, so
    /// the edges make a piece too narrow.
    fn cut(&mut self, atom: &'a Atom, edges: &[f32], width_alone: impl Fn(usize, usize) -> f32) {
        let chars = edges.len().saturating_sub(1);
        let mut from = 0;
        while from < chars {
            let room = self.limit() - self.x;
            let (to, width) = longest_piece(edges, from, room, &width_alone);
            let mut fragment = Fragment::whole(atom, self.x);
            fragment.from = from;
            fragment.to = to;
            fragment.width = width;
            self.put(fragment);
            from = to;
            if from < chars {
                self.next_row();
            }
        }
        self.x += atom.gap;
    }
}

/// Where the longest piece of a word that starts at `from` and is no wider than `room` ends,
/// and how wide it is. At least one character stays in it, however wide that is.
fn longest_piece(
    edges: &[f32],
    from: usize,
    room: f32,
    width_alone: &impl Fn(usize, usize) -> f32,
) -> (usize, f32) {
    let chars = edges.len().saturating_sub(1);
    // The positions in the one line of text give a first guess, and the pieces decide.
    let mut to = from + 1;
    while to < chars && edges[to + 1] - edges[from] <= room {
        to += 1;
    }
    let mut width = width_alone(from, to);
    while to > from + 1 && width > room {
        to -= 1;
        width = width_alone(from, to);
    }
    while to < chars {
        let longer = width_alone(from, to + 1);
        if longer > room {
            break;
        }
        (to, width) = (to + 1, longer);
    }
    (to, width)
}
