//! What one text role looks like in running text: its fonts, colours and lengths. Rich text reads
//! every colour and length of the theme here only, so a theme change is a change in one place.

use eframe::egui::{Align, Color32, FontId, Stroke, text::TextFormat};

use super::parse::SpanStyle;
use crate::theme::{TextRole, color, radius, space, stroke};

/// The size of a footnote mark, as a share of the size of the text beside it.
const FOOT_SCALE: f32 = 0.72;
/// How far a footnote mark is raised, as a share of the size of the text beside it.
const FOOT_RAISE: f32 = 1.0 / 3.0;
/// How far the middle of a citation numeral is raised above the baseline, as a share of the size
/// of the text beside it. At this height the top of the numeral is level with the tops of the
/// capitals.
const CITE_RISE: f32 = 0.45;

#[derive(Debug, Clone)]
pub(super) struct TextLook {
    pub(super) role: TextRole,
    pub(super) font: FontId,
    pub(super) strong_font: FontId,
    pub(super) italic_font: FontId,
    pub(super) strong_italic_font: FontId,
    pub(super) code_font: FontId,
    pub(super) foot_font: FontId,
    pub(super) color: Color32,
    pub(super) strong_color: Color32,
    pub(super) foot_color: Color32,
    pub(super) code_fill: Color32,
    pub(super) code_radius: f32,
    /// How far the fill of a code span reaches past its text.
    pub(super) code_pad: f32,
    pub(super) line_height: f32,
    /// Points to the em of a formula set in this text.
    pub(super) math_size: f32,
    /// How far a footnote mark is lifted above the baseline.
    pub(super) foot_raise: f32,
    /// How far the middle of a citation numeral is lifted above the baseline.
    pub(super) cite_rise: f32,
    /// Points of room added after every letter.
    pub(super) letter_spacing: f32,
    pub(super) paragraph_gap: f32,
    /// Room before a citation numeral, and between two numerals. The numeral has room of its own,
    /// so a small gap is enough to keep the box of a selected numeral off a full stop before it.
    pub(super) cite_gap: f32,
    /// Room between a footnote mark and its note.
    pub(super) small_gap: f32,
    /// Room between a list marker and its text.
    pub(super) list_gap: f32,
    /// Where each row sits in the room it has. Only a column of a table moves its rows.
    pub(super) align: Align,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct TableLook {
    pub(super) pad_x: f32,
    pub(super) pad_y: f32,
}

pub(super) fn text_look(role: TextRole) -> TextLook {
    let font = role.font();
    TextLook {
        role,
        foot_font: FontId {
            size: font.size * FOOT_SCALE,
            family: font.family.clone(),
        },
        foot_raise: font.size * FOOT_RAISE,
        cite_rise: font.size * CITE_RISE,
        letter_spacing: role.format(role.color()).extra_letter_spacing,
        font,
        strong_font: role.strong_font(),
        italic_font: role.italic_font(),
        strong_italic_font: role.strong_italic_font(),
        code_font: role.code_font(),
        color: role.color(),
        strong_color: color::TEXT,
        foot_color: color::TEXT_SECONDARY,
        code_fill: color::WELL,
        code_radius: radius::SM,
        code_pad: space::XXS,
        line_height: role.line_height(),
        math_size: role.math_size(),
        paragraph_gap: space::MD,
        cite_gap: 1.0,
        small_gap: space::XS,
        list_gap: space::SM,
        align: Align::Min,
    }
}

/// The header of a table is set in capitals with a little room between the letters. The table
/// upper-cases the words itself, because the role sets the font and the spacing but not the
/// letters.
pub(super) fn header_look() -> TextLook {
    text_look(TextRole::Caps)
}

/// The heavy line along the top and the bottom of a table.
pub(super) fn heavy_rule() -> Stroke {
    Stroke::new(stroke::HEAVY, color::RULE)
}

/// The line under the header of a table.
pub(super) fn header_rule() -> Stroke {
    Stroke::new(stroke::BORDER, color::BORDER)
}

pub(super) fn table_look() -> TableLook {
    TableLook {
        pad_x: space::MD,
        pad_y: space::XS,
    }
}

impl TextLook {
    pub(super) fn format(&self, style: SpanStyle) -> TextFormat {
        let (font_id, color) = match style {
            SpanStyle::Plain => (&self.font, self.color),
            SpanStyle::Emphasis => (&self.italic_font, self.color),
            SpanStyle::Strong => (&self.strong_font, self.strong_color),
            SpanStyle::StrongEmphasis => (&self.strong_italic_font, self.strong_color),
            SpanStyle::Code => (&self.code_font, self.color),
        };
        TextFormat {
            font_id: font_id.clone(),
            color,
            extra_letter_spacing: self.letter_spacing,
            ..TextFormat::default()
        }
    }

    pub(super) fn foot_format(&self) -> TextFormat {
        TextFormat {
            font_id: self.foot_font.clone(),
            color: self.foot_color,
            extra_letter_spacing: self.letter_spacing,
            ..TextFormat::default()
        }
    }
}
