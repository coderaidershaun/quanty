//! What one text role looks like in running text: its fonts, colours and lengths. Rich text reads
//! every colour and length of the theme here only, so a theme change is a change in one place.

use eframe::egui::{Color32, FontId, Painter, Stroke, text::TextFormat};

use super::parse::SpanStyle;
use crate::theme::{TextRole, color, hairline, radius, space};

/// The size of a footnote mark, as a share of the size of the text beside it.
const FOOT_SCALE: f32 = 0.72;
/// How far a footnote mark is raised, as a share of the size of the text beside it.
const FOOT_RAISE: f32 = 1.0 / 3.0;

#[derive(Debug, Clone)]
pub(super) struct TextLook {
    pub(super) role: TextRole,
    pub(super) font: FontId,
    pub(super) strong_font: FontId,
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
    pub(super) paragraph_gap: f32,
    /// Room before a citation chip, and between a footnote mark and its note.
    pub(super) small_gap: f32,
    /// Room between a list marker and its text.
    pub(super) list_gap: f32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct TableLook {
    pub(super) pad_x: f32,
    pub(super) pad_y: f32,
    pub(super) header_fill: Color32,
}

pub(super) fn text_look(role: TextRole) -> TextLook {
    let font = role.font();
    // SMELL: the theme keeps the colour of a role to itself, so its rule is repeated here. If
    // the theme changes the rule, this copy must follow by hand.
    let text_color = if role == TextRole::Small {
        color::TEXT_SECONDARY
    } else {
        color::TEXT
    };
    TextLook {
        role,
        foot_font: FontId {
            size: font.size * FOOT_SCALE,
            family: font.family.clone(),
        },
        foot_raise: font.size * FOOT_RAISE,
        font,
        strong_font: role.strong_font(),
        code_font: role.code_font(),
        color: text_color,
        strong_color: color::TEXT,
        foot_color: color::TEXT_MUTED,
        code_fill: color::RAISED_HOVER,
        code_radius: radius::SM,
        code_pad: space::XXS,
        line_height: role.line_height(),
        math_size: role.math_size(),
        paragraph_gap: space::MD,
        small_gap: space::XS,
        list_gap: space::SM,
    }
}

/// The look of the first row of a table: small type in the strong font.
pub(super) fn header_look() -> TextLook {
    let small = text_look(TextRole::Small);
    TextLook {
        font: small.strong_font.clone(),
        ..small
    }
}

/// The line between two rows of a table.
pub(super) fn rule(painter: &Painter) -> Stroke {
    hairline(painter, color::BORDER)
}

pub(super) fn table_look() -> TableLook {
    TableLook {
        pad_x: space::MD,
        pad_y: space::SM,
        header_fill: color::RAISED_HOVER,
    }
}

impl TextLook {
    pub(super) fn format(&self, style: SpanStyle) -> TextFormat {
        let (font_id, color) = match style {
            SpanStyle::Plain | SpanStyle::Emphasis => (&self.font, self.color),
            SpanStyle::Strong | SpanStyle::StrongEmphasis => (&self.strong_font, self.strong_color),
            SpanStyle::Code => (&self.code_font, self.color),
        };
        TextFormat {
            font_id: font_id.clone(),
            color,
            italics: matches!(style, SpanStyle::Emphasis | SpanStyle::StrongEmphasis),
            ..TextFormat::default()
        }
    }

    pub(super) fn foot_format(&self) -> TextFormat {
        TextFormat {
            font_id: self.foot_font.clone(),
            color: self.foot_color,
            ..TextFormat::default()
        }
    }
}
