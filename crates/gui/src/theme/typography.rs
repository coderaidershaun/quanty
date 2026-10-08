//! The eight text styles of the app: size, weight, line height and default colour of each.

use std::sync::Arc;

use eframe::egui::{self, Color32, FontFamily, FontId};

use super::color;
use super::fonts::{MEDIUM, SEMIBOLD};

/// A formula in a line of text is drawn this much larger than the text, so the small letters of
/// both are the same height.
const INLINE_MATH_SCALE: f32 = 1.15;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextRole {
    Title,
    Heading,
    Body,
    BodyStrong,
    Label,
    Small,
    Micro,
    Mono,
}

impl TextRole {
    const fn size(self) -> f32 {
        match self {
            TextRole::Title => 20.0,
            TextRole::Heading => 16.0,
            TextRole::Body | TextRole::BodyStrong => 14.0,
            TextRole::Label | TextRole::Mono => 13.0,
            TextRole::Small => 12.0,
            TextRole::Micro => 11.0,
        }
    }

    pub const fn color(self) -> Color32 {
        match self {
            TextRole::Small => color::TEXT_SECONDARY,
            _ => color::TEXT,
        }
    }

    pub fn font(self) -> FontId {
        let family = match self {
            TextRole::Title | TextRole::Heading | TextRole::BodyStrong | TextRole::Micro => {
                SEMIBOLD.clone()
            }
            TextRole::Label => MEDIUM.clone(),
            TextRole::Body | TextRole::Small => FontFamily::Proportional,
            TextRole::Mono => FontFamily::Monospace,
        };
        FontId::new(self.size(), family)
    }

    /// Points.
    pub const fn line_height(self) -> f32 {
        match self {
            TextRole::Title => 28.0,
            TextRole::Heading | TextRole::Body | TextRole::BodyStrong => 22.0,
            TextRole::Label | TextRole::Mono => 18.0,
            TextRole::Small => 16.0,
            TextRole::Micro => 14.0,
        }
    }

    pub fn format(self, color: Color32) -> egui::text::TextFormat {
        egui::text::TextFormat {
            font_id: self.font(),
            color,
            line_height: Some(self.line_height()),
            ..egui::text::TextFormat::default()
        }
    }

    pub fn rich(self, text: impl Into<String>) -> egui::RichText {
        egui::RichText::new(text)
            .font(self.font())
            .color(self.color())
            .line_height(Some(self.line_height()))
    }

    pub fn galley(self, ui: &egui::Ui, text: &str, color: Color32) -> Arc<egui::Galley> {
        ui.painter()
            .layout_no_wrap(text.to_owned(), self.font(), color)
    }

    pub fn strong_font(self) -> FontId {
        FontId::new(self.size(), SEMIBOLD.clone())
    }

    pub fn code_font(self) -> FontId {
        FontId::new(self.size() - 1.0, FontFamily::Monospace)
    }

    pub const fn math_size(self) -> f32 {
        self.size() * INLINE_MATH_SCALE
    }
}
