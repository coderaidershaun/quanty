//! Breaks the atoms of one line into rows: greedy, with no break inside a run of atoms that
//! have no space between them, and a cut for a word that is wider than a whole row.

use super::atom::{Atom, AtomKind, MeasuredLine, unit_width};

/// A row may be this much longer than the width: words are measured in one line of text and
/// drawn in several, and the two differ by a fraction of a point.
const SLACK: f32 = 0.25;
/// A formula that is wider than its row is not drawn smaller than this share of its size.
const SMALLEST_SCALE: f32 = 0.05;

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

/// The rows of one line.
pub(super) fn break_line(line: &MeasuredLine, wrap: f32) -> Vec<Vec<Fragment<'_>>> {
    let mut breaker = Breaker {
        rows: Vec::new(),
        row: Vec::new(),
        x: line.indent,
        indent: line.indent,
        wrap,
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
            AtomKind::Word { edges, .. } => self.cut(atom, edges),
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

    /// Cuts a word at the last character that fits, as many times as it takes.
    fn cut(&mut self, atom: &'a Atom, edges: &[f32]) {
        let chars = edges.len().saturating_sub(1);
        let mut from = 0;
        while from < chars {
            let room = self.limit() - self.x;
            let mut to = from + 1;
            while to < chars && edges[to + 1] - edges[from] <= room {
                to += 1;
            }
            let mut fragment = Fragment::whole(atom, self.x);
            fragment.from = from;
            fragment.to = to;
            fragment.width = edges[to] - edges[from];
            self.put(fragment);
            from = to;
            if from < chars {
                self.next_row();
            }
        }
        self.x += atom.gap;
    }
}
