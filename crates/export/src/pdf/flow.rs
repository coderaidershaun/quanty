//! Text that runs along lines: what a line is made of, where a line ends, and how a line is drawn.
//! A line only ends at a space. A word that is wider than a row is cut, and a formula that is
//! wider than a row is made smaller.

use std::rc::Rc;

use krilla::image::Image;

use super::faces::{Faces, Typeface};
use super::pages::{Chunk, INK_SECONDARY, INK_TEXT, Item};
use super::read::{Span, SpanKind};
use crate::math::{Display, Formula, Formulas};

/// A formula is set a little larger than the text beside it, as it is in the window.
const INLINE_FORMULA_RATIO: f32 = 1.12;
const CODE_RATIO: f32 = 0.92;
const NOTE_MARK_SIZE: f32 = 7.0;
const NOTE_MARK_RAISE: f32 = 3.5;

/// How a block sets its text: the size, the distance between two baselines, and the ink.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Look {
    pub(super) size: f32,
    pub(super) leading: f32,
    pub(super) ink: u8,
    pub(super) is_strong: bool,
    pub(super) is_italic: bool,
}

/// Body text, and the smaller text of captions, tables and the list of sources.
pub(super) const BODY: Look = Look::new(11.0, 15.5);
pub(super) const SMALL: Look = Look::new(9.5, 13.0);
/// The line of a note, such as the text of a footnote, and the caption of a formula.
pub(super) const NOTE: Look = Look::new(9.0, 12.5);

impl Look {
    pub(super) const fn new(size: f32, leading: f32) -> Look {
        Look {
            size,
            leading,
            ink: INK_TEXT,
            is_strong: false,
            is_italic: false,
        }
    }

    pub(super) const fn strong(self) -> Look {
        Look {
            is_strong: true,
            ..self
        }
    }

    pub(super) const fn italic(self) -> Look {
        Look {
            is_italic: true,
            ..self
        }
    }

    pub(super) const fn secondary(self) -> Look {
        Look {
            ink: INK_SECONDARY,
            ..self
        }
    }

    fn style(self) -> Style {
        let face = match (self.is_strong, self.is_italic) {
            (false, false) => Typeface::Serif,
            (true, false) => Typeface::SerifStrong,
            (false, true) => Typeface::SerifItalic,
            (true, true) => Typeface::SerifStrongItalic,
        };
        Style {
            face,
            size: self.size,
            ink: self.ink,
            raise: 0.0,
        }
    }
}

/// The face, size, ink and height above the baseline that a piece of text is drawn with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Style {
    pub(super) face: Typeface,
    pub(super) size: f32,
    pub(super) ink: u8,
    pub(super) raise: f32,
}

/// What a line is made of.
#[derive(Clone)]
pub(super) enum Atom {
    Text {
        text: String,
        style: Style,
        width: f32,
    },
    /// The only place a line may end.
    Space {
        style: Style,
        width: f32,
    },
    Formula {
        formula: Rc<Formula>,
        /// Points to the em.
        em: f32,
    },
    Break,
}

impl Atom {
    pub(super) fn width(&self) -> f32 {
        match self {
            Atom::Text { width, .. } | Atom::Space { width, .. } => *width,
            Atom::Formula { formula, em } => formula.width_em as f32 * em,
            Atom::Break => 0.0,
        }
    }
}

pub(super) struct Line {
    pub(super) atoms: Vec<Atom>,
    pub(super) width: f32,
    pub(super) ascent: f32,
    pub(super) descent: f32,
}

impl Line {
    pub(super) fn height(&self) -> f32 {
        self.ascent + self.descent
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Align {
    Left,
    Center,
    Right,
}

/// What every block is laid out with: the fonts, and the formulas typeset so far.
pub(super) struct Layout<'a> {
    pub(super) faces: &'a Faces,
    pub(super) formulas: Formulas,
}

impl<'a> Layout<'a> {
    pub(super) fn new(faces: &'a Faces) -> Layout<'a> {
        Layout {
            faces,
            formulas: Formulas::default(),
        }
    }

    pub(super) fn atoms(&mut self, spans: &[Span], look: Look) -> Vec<Atom> {
        let mut atoms = Vec::new();
        for span in spans {
            let text_style = Look {
                is_strong: look.is_strong || span.is_strong,
                is_italic: look.is_italic || span.is_emphasised,
                ..look
            }
            .style();
            let code_style = Style {
                face: Typeface::Code,
                size: look.size * CODE_RATIO,
                ..text_style
            };
            match &span.kind {
                SpanKind::Text(text) => self.words(&mut atoms, text, text_style),
                SpanKind::Code(code) => self.words(&mut atoms, code, code_style),
                SpanKind::Math(latex) => match self.formulas.get(latex, Display::Inline) {
                    Some(formula) => atoms.push(Atom::Formula {
                        formula,
                        em: look.size * INLINE_FORMULA_RATIO,
                    }),
                    None => atoms.push(self.text(&one_line(latex), code_style)),
                },
                SpanKind::NoteMark(label) => {
                    let style = Style {
                        size: NOTE_MARK_SIZE,
                        raise: NOTE_MARK_RAISE,
                        ..text_style
                    };
                    atoms.push(self.text(label, style));
                }
                SpanKind::Cite(mark) => {
                    let style = Style {
                        ink: INK_SECONDARY,
                        ..text_style
                    };
                    atoms.push(self.space(style));
                    atoms.push(self.text(mark, style));
                }
                SpanKind::LineBreak => atoms.push(Atom::Break),
            }
        }
        atoms
    }

    fn words(&self, atoms: &mut Vec<Atom>, text: &str, style: Style) {
        let mut word = String::new();
        let mut after_space = false;
        for character in text.chars() {
            if character.is_whitespace() {
                if !word.is_empty() {
                    atoms.push(self.text(&std::mem::take(&mut word), style));
                }
                if !after_space {
                    atoms.push(self.space(style));
                    after_space = true;
                }
            } else {
                after_space = false;
                word.push(character);
            }
        }
        if !word.is_empty() {
            atoms.push(self.text(&word, style));
        }
    }

    fn text(&self, text: &str, style: Style) -> Atom {
        Atom::Text {
            width: self.faces.width(text, style.face, style.size),
            text: text.to_owned(),
            style,
        }
    }

    fn space(&self, style: Style) -> Atom {
        Atom::Space {
            width: self.faces.width(" ", style.face, style.size),
            style,
        }
    }

    /// How far a line of this look rises above its baseline and falls below it.
    pub(super) fn line_box(&self, look: Look) -> (f32, f32) {
        let (rise, fall) = self.faces.extent(Typeface::Serif);
        let ascent = (look.leading - (rise + fall) * look.size) / 2.0 + rise * look.size;
        (ascent, look.leading - ascent)
    }

    /// One chunk for each line, placed in a row of `row` points that starts `x` points from the
    /// left margin. The first two and the last two lines stay together, so that no page starts or
    /// ends with a single line of a block.
    pub(super) fn chunks(&self, lines: &[Line], x: f32, row: f32, align: Align) -> Vec<Chunk> {
        let mut chunks: Vec<Chunk> = lines
            .iter()
            .map(|line| {
                let mut items = Vec::new();
                let place = (x, 0.0, row);
                self.draw_lines(std::slice::from_ref(line), place, align, &mut items);
                Chunk::new(line.height(), items)
            })
            .collect();
        keep_lines_together(&mut chunks);
        chunks
    }

    /// The lines one under the other from `top`, in a row of `row` points that starts `x` points
    /// from the left margin. Gives back how tall they are together.
    pub(super) fn draw_lines(
        &self,
        lines: &[Line],
        (x, top, row): (f32, f32, f32),
        align: Align,
        items: &mut Vec<Item>,
    ) -> f32 {
        let mut y = top;
        for line in lines {
            let start = match align {
                Align::Left => x,
                Align::Center => x + (row - line.width) / 2.0,
                Align::Right => x + row - line.width,
            };
            self.draw_line(line, start, y + line.ascent, items);
            y += line.height();
        }
        y - top
    }

    /// Draws one piece of text with its baseline `baseline` points below the top of the chunk, and
    /// gives back how wide it is.
    pub(super) fn draw_text(
        &self,
        text: &str,
        look: Look,
        at: (f32, f32),
        items: &mut Vec<Item>,
    ) -> f32 {
        self.push_runs(text, look.style(), at, items)
    }

    /// Puts the shaped runs of `text` in `items` from `x`, and gives back how wide they are
    /// together.
    fn push_runs(
        &self,
        text: &str,
        style: Style,
        (x, baseline): (f32, f32),
        items: &mut Vec<Item>,
    ) -> f32 {
        let mut end = x;
        for run in self.faces.shape(text, style.face) {
            let width = run.width(style.size);
            items.push(Item::Glyphs {
                x: end,
                y: baseline - style.raise,
                run,
                size: style.size,
                ink: style.ink,
            });
            end += width;
        }
        end - x
    }

    /// Draws the line from `x` with its baseline `baseline` points below the top of the chunk.
    /// Text of one style that runs on, spaces included, is drawn as one run, so that it can be
    /// read back from the file with its spaces.
    pub(super) fn draw_line(&self, line: &Line, x: f32, baseline: f32, items: &mut Vec<Item>) {
        let mut segments: Vec<Segment<'_>> = Vec::new();
        for atom in &line.atoms {
            match atom {
                Atom::Text { text, style, .. } => push_text(&mut segments, *style, text),
                Atom::Space { style, .. } => push_text(&mut segments, *style, " "),
                Atom::Formula { formula, em } => segments.push(Segment::Formula(formula, *em)),
                Atom::Break => {}
            }
        }
        let mut x = x;
        for segment in segments {
            match segment {
                Segment::Text(style, text) => {
                    x += self.push_runs(&text, style, (x, baseline), items);
                }
                Segment::Formula(formula, em) => {
                    items.extend(formula_picture(formula, em, (x, baseline)));
                    x += formula.width_em as f32 * em;
                }
            }
        }
    }
}

/// The picture of a formula, with the left end of its box `x` points from the left margin and
/// its baseline `baseline` points below the top of the chunk. `None` when the picture cannot be
/// read, and then the formula is left out.
pub(super) fn formula_picture(
    formula: &Formula,
    em: f32,
    (x, baseline): (f32, f32),
) -> Option<Item> {
    let bleed = formula.bleed_em as f32 * em;
    let image = Image::from_png(formula.png.clone().into(), true).ok()?;
    Some(Item::Picture {
        x: x - bleed,
        y: baseline - formula.ascent_em as f32 * em - bleed,
        width: formula.picture_width_em() as f32 * em,
        height: formula.picture_height_em() as f32 * em,
        image,
    })
}

/// A piece of a line that is drawn in one go.
enum Segment<'a> {
    Text(Style, String),
    Formula(&'a Formula, f32),
}

fn push_text(segments: &mut Vec<Segment<'_>>, style: Style, text: &str) {
    if let Some(Segment::Text(last, run)) = segments.last_mut()
        && *last == style
    {
        run.push_str(text);
    } else {
        segments.push(Segment::Text(style, text.to_owned()));
    }
}

/// Keeps the first two lines together and the last two lines together.
pub(super) fn keep_lines_together(chunks: &mut [Chunk]) {
    let count = chunks.len();
    if count >= 2 {
        chunks[0].keep_with_next = 1;
        chunks[count - 2].keep_with_next = 1;
    }
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
