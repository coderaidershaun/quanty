//! What a caller holds of a formula: how to ask for one, what comes back, and how to paint it.

use eframe::egui::{self, emath::GuiRounding as _};

use crate::theme::{TextRole, size};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Display {
    Inline,
    Block,
}

/// One formula: LaTeX with no delimiters around it, and the size to set it at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MathRef<'a> {
    pub latex: &'a str,
    pub display: Display,
    /// Points per em: the font size of the formula.
    pub size: f32,
}

impl<'a> MathRef<'a> {
    pub fn block(latex: &'a str) -> Self {
        MathRef {
            latex,
            display: Display::Block,
            size: size::MATH_DISPLAY,
        }
    }

    pub fn inline(latex: &'a str, beside: TextRole) -> Self {
        MathRef {
            latex,
            display: Display::Inline,
            size: beside.math_size(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MathState {
    Loading,
    Ready(MathImage),
    /// The LaTeX cannot be typeset. Draw `math.latex` in the code font instead.
    Failed,
}

/// A typeset formula: white ink on a clear ground. Tint it when painting. All lengths are in
/// points. Valid only in the frame that returned it: do not store it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MathImage {
    pub texture: egui::TextureId,
    /// Size of the whole texture. It is the box plus `bleed` on every side.
    pub texture_size: egui::Vec2,
    /// Clear margin around the box inside the texture. Ink can reach a little outside the box.
    pub bleed: f32,
    /// Width of the formula's box. Lay out with this.
    pub width: f32,
    pub ascent: f32,
    pub descent: f32,
}

impl MathImage {
    pub fn size(&self) -> egui::Vec2 {
        egui::vec2(self.width, self.ascent + self.descent)
    }

    /// At `scale` 1.0 each pixel of the picture lands on one pixel of the screen.
    pub fn paint(
        &self,
        painter: &egui::Painter,
        baseline_left: egui::Pos2,
        scale: f32,
        tint: egui::Color32,
    ) {
        const WHOLE_TEXTURE: egui::Rect =
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        let mut corner = baseline_left - egui::vec2(self.bleed, self.ascent + self.bleed) * scale;
        if scale == 1.0 {
            // A picture that starts between two pixels of the screen is drawn blurred.
            corner = corner.round_to_pixels(painter.ctx().pixels_per_point());
        }
        let rect = egui::Rect::from_min_size(corner, self.texture_size * scale);
        painter.image(self.texture, rect, WHOLE_TEXTURE, tint);
    }
}
