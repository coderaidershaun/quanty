//! Where the lines of a text end: the atoms are placed one after the other, and a line ends at the
//! last space that fits. A word wider than a row is cut where the row ends, and a formula wider
//! than a row is made smaller.

use super::flow::{Atom, Layout, Line, Look};

/// Room kept above and below a formula that is taller than the line it stands in.
const FORMULA_ROOM: f32 = 1.0;

impl Layout<'_> {
    /// The atoms as lines no wider than `row`.
    pub(super) fn lines(&self, atoms: Vec<Atom>, row: f32, look: Look) -> Vec<Line> {
        let mut breaker = Breaker {
            layout: self,
            row,
            text_box: self.line_box(look),
            lines: Vec::new(),
            atoms: Vec::new(),
            width: 0.0,
            space: None,
        };
        let mut unit: Vec<Atom> = Vec::new();
        for atom in atoms {
            match atom {
                Atom::Space { .. } => {
                    breaker.place(std::mem::take(&mut unit));
                    breaker.space = Some(atom);
                }
                Atom::Break => {
                    breaker.place(std::mem::take(&mut unit));
                    breaker.close();
                }
                atom => unit.push(atom),
            }
        }
        breaker.place(unit);
        if !breaker.atoms.is_empty() || breaker.lines.is_empty() {
            breaker.close();
        }
        breaker.lines
    }
}

/// Fills lines with the atoms that are placed one after the other.
struct Breaker<'a, 'b> {
    layout: &'a Layout<'b>,
    row: f32,
    text_box: (f32, f32),
    lines: Vec<Line>,
    atoms: Vec<Atom>,
    width: f32,
    /// The space that came before the next unit, which is dropped at the start of a line.
    space: Option<Atom>,
}

impl Breaker<'_, '_> {
    /// Puts a run of atoms with no space between them on the line, or on the next one when it does
    /// not fit.
    fn place(&mut self, unit: Vec<Atom>) {
        if unit.is_empty() {
            return;
        }
        let width: f32 = unit.iter().map(Atom::width).sum();
        if width > self.row {
            let mut pieces = self.fit(unit).into_iter().peekable();
            while let Some(piece) = pieces.next() {
                self.place_whole(piece);
                if pieces.peek().is_some() {
                    self.close();
                }
            }
        } else {
            self.place_whole(unit);
        }
    }

    fn place_whole(&mut self, unit: Vec<Atom>) {
        let width: f32 = unit.iter().map(Atom::width).sum();
        let space = self.space.take();
        let space_width = space.as_ref().map_or(0.0, Atom::width);
        if !self.atoms.is_empty() && self.width + space_width + width > self.row + 0.01 {
            self.close();
        }
        if !self.atoms.is_empty()
            && let Some(space) = space
        {
            self.width += space.width();
            self.atoms.push(space);
        }
        self.width += width;
        self.atoms.extend(unit);
    }

    /// A run that is wider than a row, as one run or as several that each fit: a formula in it is
    /// made smaller when the text beside it leaves room, and text is cut where the row ends.
    fn fit(&self, mut unit: Vec<Atom>) -> Vec<Vec<Atom>> {
        let formula_width: f32 = unit
            .iter()
            .filter(|atom| matches!(atom, Atom::Formula { .. }))
            .map(Atom::width)
            .sum();
        let text_width: f32 = unit.iter().map(Atom::width).sum::<f32>() - formula_width;
        if formula_width > 0.0 && text_width < self.row * 0.9 {
            let scale = (self.row - text_width) / formula_width;
            for atom in &mut unit {
                if let Atom::Formula { em, .. } = atom {
                    *em *= scale;
                }
            }
            return vec![unit];
        }
        let mut pieces: Vec<Vec<Atom>> = Vec::new();
        let mut piece: Vec<Atom> = Vec::new();
        let mut used = 0.0;
        for atom in unit {
            match atom {
                Atom::Text { text, style, width } if used + width > self.row => {
                    let mut part = String::new();
                    let mut part_width = 0.0;
                    for character in text.chars() {
                        let character_width = self.layout.faces.width(
                            character.encode_utf8(&mut [0; 4]),
                            style.face,
                            style.size,
                        );
                        if used + part_width + character_width > self.row
                            && (used > 0.0 || !part.is_empty())
                        {
                            if !part.is_empty() {
                                piece.push(Atom::Text {
                                    text: std::mem::take(&mut part),
                                    style,
                                    width: part_width,
                                });
                            }
                            pieces.push(std::mem::take(&mut piece));
                            used = 0.0;
                            part_width = 0.0;
                        }
                        part.push(character);
                        part_width += character_width;
                    }
                    if !part.is_empty() {
                        piece.push(Atom::Text {
                            text: part,
                            style,
                            width: part_width,
                        });
                        used += part_width;
                    }
                }
                atom => {
                    let width = atom.width();
                    if used + width > self.row && used > 0.0 {
                        pieces.push(std::mem::take(&mut piece));
                        used = 0.0;
                    }
                    used += width;
                    piece.push(atom);
                }
            }
        }
        pieces.push(piece);
        pieces
    }

    fn close(&mut self) {
        self.space = None;
        let (mut ascent, mut descent) = self.text_box;
        for atom in &self.atoms {
            if let Atom::Formula { formula, em } = atom {
                ascent = ascent.max(formula.ascent_em as f32 * em + FORMULA_ROOM);
                descent = descent.max(formula.descent_em as f32 * em + FORMULA_ROOM);
            }
        }
        self.lines.push(Line {
            atoms: std::mem::take(&mut self.atoms),
            width: std::mem::take(&mut self.width),
            ascent,
            descent,
        });
    }
}
