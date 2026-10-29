//! The eleven text roles of the app: family, size, line height, colour and letter spacing of each.

use std::sync::Arc;

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{self, Color32, FontFamily, FontId};

use super::Palette;
use super::fonts::{
    SANS_MEDIUM, SANS_SEMIBOLD, SERIF, SERIF_DISPLAY, SERIF_ITALIC, SERIF_SEMIBOLD,
    SERIF_SEMIBOLD_ITALIC,
};

/// A formula in a line of text is drawn this much larger than the text, so the small letters of
/// both are the same height.
const INLINE_MATH_SCALE: f32 = 1.12;

/// Points of room added after every letter of a role that is set in tracked capitals.
const CAPS_SPACING: f32 = 1.1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextRole {
    Title,
    Heading,
    /// A heading inside content, in the accent colour of the theme so that it stands apart from
    /// the body text around it.
    Subheading,
    Body,
    BodyStrong,
    Caption,
    Label,
    Small,
    Micro,
    /// Tracked capitals for the few short labels that name a kind. The text is not changed to
    /// capitals here: a widget that paints it upper-cases it first.
    Caps,
    Mono,
}

impl TextRole {
    const fn size(self) -> f32 {
        match self {
            TextRole::Title => 24.0,
            TextRole::Heading | TextRole::Subheading => 17.0,
            TextRole::Body | TextRole::BodyStrong => 16.0,
            TextRole::Caption | TextRole::Label | TextRole::Mono => 13.0,
            TextRole::Small => 12.0,
            TextRole::Micro | TextRole::Caps => 11.0,
        }
    }

    pub const fn color(self, palette: &Palette) -> Color32 {
        match self {
            TextRole::Title | TextRole::Heading => palette.text_heading,
            TextRole::Subheading => palette.text_subheading,
            TextRole::Caption | TextRole::Small | TextRole::Caps => palette.text_secondary,
            TextRole::Body
            | TextRole::BodyStrong
            | TextRole::Label
            | TextRole::Micro
            | TextRole::Mono => palette.text,
        }
    }

    pub fn font(self) -> FontId {
        let family = match self {
            TextRole::Title => SERIF_DISPLAY.clone(),
            TextRole::Heading | TextRole::Caption => SERIF_ITALIC.clone(),
            TextRole::Body => SERIF.clone(),
            TextRole::BodyStrong | TextRole::Subheading => SERIF_SEMIBOLD.clone(),
            TextRole::Label | TextRole::Caps => SANS_MEDIUM.clone(),
            TextRole::Small => FontFamily::Proportional,
            TextRole::Micro => SANS_SEMIBOLD.clone(),
            TextRole::Mono => FontFamily::Monospace,
        };
        FontId::new(self.size(), family)
    }

    /// Points.
    pub const fn line_height(self) -> f32 {
        match self {
            TextRole::Title => 30.0,
            TextRole::Heading | TextRole::Subheading | TextRole::Body | TextRole::BodyStrong => {
                24.0
            }
            TextRole::Caption | TextRole::Label | TextRole::Mono => 18.0,
            TextRole::Small => 16.0,
            TextRole::Micro | TextRole::Caps => 14.0,
        }
    }

    /// Points.
    const fn letter_spacing(self) -> f32 {
        match self {
            TextRole::Caps => CAPS_SPACING,
            _ => 0.0,
        }
    }

    pub fn format(self, color: Color32) -> TextFormat {
        TextFormat {
            font_id: self.font(),
            color,
            line_height: Some(self.line_height()),
            extra_letter_spacing: self.letter_spacing(),
            ..TextFormat::default()
        }
    }

    pub fn rich(self, ui: &egui::Ui, text: impl Into<String>) -> egui::RichText {
        egui::RichText::new(text)
            .font(self.font())
            .color(self.color(super::palette(ui.ctx())))
            .line_height(Some(self.line_height()))
            .extra_letter_spacing(self.letter_spacing())
    }

    pub fn galley(self, ui: &egui::Ui, text: &str, color: Color32) -> Arc<egui::Galley> {
        let job = LayoutJob::simple_format(text.to_owned(), self.format(color));
        ui.painter().layout_job(job)
    }

    /// The font of an emphasis in running text: the serif italic at the size of the role.
    pub fn italic_font(self) -> FontId {
        FontId::new(self.size(), SERIF_ITALIC.clone())
    }

    /// The font of an emphasis that is also strong, at the size of the role.
    pub fn strong_italic_font(self) -> FontId {
        FontId::new(self.size(), SERIF_SEMIBOLD_ITALIC.clone())
    }

    /// The weight that a role takes when a word in it is strong. A code role has no heavier
    /// weight, so it stays as it is.
    pub fn strong_font(self) -> FontId {
        let family = match self {
            TextRole::Title
            | TextRole::Heading
            | TextRole::Subheading
            | TextRole::Body
            | TextRole::BodyStrong
            | TextRole::Caption => SERIF_SEMIBOLD.clone(),
            TextRole::Label | TextRole::Small | TextRole::Micro | TextRole::Caps => {
                SANS_SEMIBOLD.clone()
            }
            TextRole::Mono => FontFamily::Monospace,
        };
        FontId::new(self.size(), family)
    }

    pub fn code_font(self) -> FontId {
        FontId::new(self.size() - 1.0, FontFamily::Monospace)
    }

    pub const fn math_size(self) -> f32 {
        self.size() * INLINE_MATH_SCALE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_headings_stand_apart_from_body_text_in_both_palettes() {
        for palette in [&Palette::LIGHT, &Palette::DARK] {
            let heading = TextRole::Subheading.color(palette);
            assert_ne!(heading, TextRole::Body.color(palette));
            assert_ne!(heading, palette.text_heading);
        }
    }
}
