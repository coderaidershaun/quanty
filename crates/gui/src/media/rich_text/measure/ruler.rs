//! Where every piece of a block sits among the glyphs of the one line of text that measures
//! the block.

use eframe::egui::{self, Color32, Ui, text::TextFormat};

use super::pieces::LinePieces;
use crate::media::rich_text::parse::SpanStyle;

/// All the words of the text, in the order of the one line of text that they make, and where
/// each piece sits in it.
pub(super) fn in_one_line(lines: &[LinePieces<'_>]) -> (Vec<(String, TextFormat)>, Vec<Slot>) {
    let mut parts = Vec::new();
    let mut slots = Vec::new();
    let mut glyphs = 0;
    for line in lines {
        let plain = line.look.format(SpanStyle::Plain);
        // A marker has a space after it in the one line of text, so that it is measured apart
        // from the first word.
        let marker = line.marker.iter().map(|marker| (marker, true));
        let pieces = line
            .pieces
            .iter()
            .map(|piece| (piece, piece.has_space_after));
        let texts: Vec<(&str, &TextFormat, bool)> = marker
            .chain(pieces)
            .filter_map(|(piece, is_spaced)| {
                let (text, format) = piece.as_text()?;
                Some((text, format, is_spaced))
            })
            .collect();
        for (index, &(text, format, is_spaced)) in texts.iter().enumerate() {
            let is_last = index + 1 == texts.len();
            // The last piece of a line gets a space too, so that it never touches the next line.
            let space = if is_spaced || is_last { " " } else { "" };
            // A space is set in the plain font unless the next word is in the same font as this
            // one, so that a code word is not followed by the wide space of the code font.
            let next_font = texts.get(index + 1).map(|(_, next, _)| &next.font_id);
            let space_format = if next_font == Some(&format.font_id) {
                format
            } else {
                &plain
            };
            parts.push((text.to_owned(), format.clone()));
            if !space.is_empty() {
                parts.push((space.to_owned(), space_format.clone()));
            }
            let chars = text.chars().count();
            let after = glyphs + chars + space.len();
            slots.push(Slot {
                start: glyphs,
                chars,
                is_spaced,
                is_joined: !is_spaced && !is_last,
                after,
            });
            glyphs = after;
        }
    }
    (parts, slots)
}

/// Where a piece sits among the glyphs of the one line of text.
pub(super) struct Slot {
    start: usize,
    chars: usize,
    /// A space of the text follows it, or it is a marker.
    is_spaced: bool,
    /// The next piece of the same line starts right after it.
    is_joined: bool,
    /// The first glyph after the piece and the space that follows it, if any.
    after: usize,
}

/// The positions of the glyphs of the one line of text.
pub(super) struct Ruler {
    lefts: Vec<f32>,
    rights: Vec<f32>,
    end: f32,
}

impl Ruler {
    /// The positions that the layout gave, when it made one glyph for each character. Some
    /// scripts that run right to left make more, and then no position can be told from the
    /// number of a character.
    pub(super) fn of(galley: &egui::Galley) -> Option<Ruler> {
        let glyphs = galley.rows.first().map_or(&[][..], |row| &row.row.glyphs);
        if glyphs.len() != galley.job.text.chars().count() {
            return None;
        }
        let lefts: Vec<f32> = glyphs.iter().map(|glyph| glyph.pos.x).collect();
        let rights: Vec<f32> = glyphs
            .iter()
            .map(|glyph| glyph.pos.x + glyph.advance_width)
            .collect();
        let end = rights.last().copied().unwrap_or(0.0);
        Some(Ruler { lefts, rights, end })
    }

    /// Lays out every part of the text alone, and spreads the characters of each part evenly
    /// over its width. A place inside a part is then only a guess, which is enough, because a
    /// word that is cut is measured again piece by piece.
    pub(super) fn by_part(ui: &Ui, parts: &[(String, TextFormat)]) -> Ruler {
        let (mut lefts, mut rights, mut x) = (Vec::new(), Vec::new(), 0.0);
        for (text, format) in parts {
            let galley =
                ui.painter()
                    .layout_no_wrap(text.clone(), format.font_id.clone(), Color32::WHITE);
            let width = galley.size().x;
            let chars = text.chars().count();
            for index in 0..chars {
                lefts.push(x + width * index as f32 / chars as f32);
                rights.push(x + width * (index + 1) as f32 / chars as f32);
            }
            x += width;
        }
        Ruler {
            lefts,
            rights,
            end: x,
        }
    }

    fn left(&self, glyph: usize) -> f32 {
        self.lefts.get(glyph).copied().unwrap_or(self.end)
    }

    fn right(&self, glyph: usize) -> f32 {
        self.rights.get(glyph).copied().unwrap_or(self.end)
    }

    /// The width of a piece and the room after it. Pieces that touch add up to the distance
    /// between their starts, so a line is as long as the text was laid out.
    pub(super) fn measure(&self, slot: &Slot) -> (f32, f32) {
        let left = self.left(slot.start);
        let last = (slot.start + slot.chars).saturating_sub(1);
        let extent = (self.right(last) - left).max(0.0);
        let advance = self.left(slot.after) - left;
        match (slot.is_spaced, slot.is_joined) {
            (true, _) => (extent, (advance - extent).max(0.0)),
            (false, true) => (advance.max(extent), 0.0),
            (false, false) => (extent, 0.0),
        }
    }

    pub(super) fn edges(&self, slot: &Slot, width: f32) -> Vec<f32> {
        let left = self.left(slot.start);
        let mut edges: Vec<f32> = (0..slot.chars)
            .map(|index| (self.left(slot.start + index) - left).clamp(0.0, width))
            .collect();
        edges.push(width);
        edges
    }
}
