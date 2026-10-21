//! The pieces of a block of Markdown as chunks of a page: paragraphs, headings, list items,
//! quotes, notes and rules.

use super::TEXT_WIDTH;
use super::flow::{Align, BODY, Layout, Look, NOTE};
use super::pages::{Chunk, INK_RULE, Item};
use super::read::{Piece, Span, read};

/// How far a list, or a quote inside it, moves the text in.
pub(super) const INDENT: f32 = 14.0;
const PARAGRAPH_AFTER: f32 = 7.0;
pub(super) const ITEM_AFTER: f32 = 3.0;
const RULE_THICKNESS: f32 = 0.5;
const RULE_ROOM: f32 = 6.0;
/// A heading stays with this many lines of what follows it.
const HEADING_KEEPS: usize = 2;

impl Layout<'_> {
    /// The chunks of `markdown`. The marks of the sources in `cites` follow its last paragraph
    /// or list item, or stand on a line of their own when it ends with something else.
    pub(super) fn markdown(&mut self, markdown: &str, cites: &[usize]) -> Vec<Chunk> {
        let mut pieces = read(markdown);
        if !cites.is_empty() {
            let marks: Vec<Span> = cites.iter().map(|&number| Span::cite(number)).collect();
            match pieces.last_mut() {
                Some(Piece::Paragraph { spans, .. } | Piece::Item { spans, .. }) => {
                    spans.extend(marks);
                }
                _ => pieces.push(Piece::Paragraph {
                    spans: marks,
                    indent: 0,
                    is_quoted: false,
                }),
            }
        }
        let mut chunks = Vec::new();
        for (at, piece) in pieces.iter().enumerate() {
            let is_before_item = matches!(pieces.get(at + 1), Some(Piece::Item { .. }));
            chunks.extend(self.piece(piece, is_before_item));
        }
        chunks
    }

    fn piece(&mut self, piece: &Piece, is_before_item: bool) -> Vec<Chunk> {
        match piece {
            Piece::Paragraph {
                spans,
                indent,
                is_quoted,
            } => {
                let look = if *is_quoted { BODY.secondary() } else { BODY };
                let x = INDENT * *indent as f32;
                let mut chunks = self.set(spans, look, (x, TEXT_WIDTH - x), Align::Left);
                space(&mut chunks, 0.0, PARAGRAPH_AFTER);
                chunks
            }
            Piece::Heading { level, spans } => self.heading(*level, spans),
            Piece::Item {
                marker,
                spans,
                indent,
            } => {
                let x = INDENT * *indent as f32;
                let mut chunks = self.hanging(marker, spans, BODY, (x, INDENT));
                let after = if is_before_item {
                    ITEM_AFTER
                } else {
                    PARAGRAPH_AFTER
                };
                space(&mut chunks, 0.0, after);
                chunks
            }
            Piece::Code { code, language } => self.code(code, language.as_deref(), Vec::new()),
            Piece::Table(table) => self.table(table),
            Piece::Formula(latex) => self.formula(latex, None, Vec::new()),
            Piece::Rule => {
                let rule = Item::Rule {
                    x1: 0.0,
                    x2: TEXT_WIDTH,
                    y: RULE_THICKNESS / 2.0,
                    thickness: RULE_THICKNESS,
                    ink: INK_RULE,
                };
                let mut chunk = Chunk::new(RULE_THICKNESS, vec![rule]);
                chunk.before = RULE_ROOM;
                chunk.after = RULE_ROOM;
                vec![chunk]
            }
            Piece::Note { label, spans } => {
                let mut with_label = vec![Span::text(&format!("{label}. "))];
                with_label.extend(spans.iter().cloned());
                let mut chunks = self.set(&with_label, NOTE, (0.0, TEXT_WIDTH), Align::Left);
                space(&mut chunks, 0.0, ITEM_AFTER);
                chunks
            }
        }
    }

    /// A heading, kept with the first lines of what follows it.
    pub(super) fn heading(&mut self, level: u8, spans: &[Span]) -> Vec<Chunk> {
        let (look, before, after) = if level <= 2 {
            (Look::new(13.5, 18.0).strong(), 12.0, 4.0)
        } else {
            (Look::new(11.5, 16.0).strong(), 8.0, 3.0)
        };
        let mut chunks = self.set(spans, look, (0.0, TEXT_WIDTH), Align::Left);
        let count = chunks.len();
        for (at, chunk) in chunks.iter_mut().enumerate() {
            chunk.keep_with_next = count - 1 - at + HEADING_KEEPS;
        }
        space(&mut chunks, before, after);
        chunks
    }

    /// Text in a row of `row` points that starts `x` points from the left margin.
    pub(super) fn set(
        &mut self,
        spans: &[Span],
        look: Look,
        (x, row): (f32, f32),
        align: Align,
    ) -> Vec<Chunk> {
        let atoms = self.atoms(spans, look);
        if atoms.is_empty() {
            return Vec::new();
        }
        let lines = self.lines(atoms, row, look);
        self.chunks(&lines, x, row, align)
    }

    /// Text with a marker in the margin beside its first line. `(x, indent)`: the marker stands
    /// `x` points from the left margin and the text `indent` points after it.
    pub(super) fn hanging(
        &mut self,
        marker: &str,
        spans: &[Span],
        look: Look,
        (x, indent): (f32, f32),
    ) -> Vec<Chunk> {
        let mut chunks = self.set(
            spans,
            look,
            (x + indent, TEXT_WIDTH - x - indent),
            Align::Left,
        );
        if let Some(first) = chunks.first_mut() {
            let (baseline, _) = self.line_box(look);
            let mut items = Vec::new();
            self.draw_text(marker, look, (x, baseline), &mut items);
            items.append(&mut first.items);
            first.items = items;
        }
        chunks
    }
}

/// Wishes for the space above the first chunk and below the last.
pub(super) fn space(chunks: &mut [Chunk], before: f32, after: f32) {
    if let Some(first) = chunks.first_mut() {
        first.before = first.before.max(before);
    }
    if let Some(last) = chunks.last_mut() {
        last.after = last.after.max(after);
    }
}
