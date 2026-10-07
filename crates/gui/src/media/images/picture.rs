//! What a caller holds for a picture: whether it is loading, ready or failed, and for a ready
//! one the stored sizes to paint from.

use eframe::egui;

use super::chain::MAX_LEVELS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFailure {
    /// No file at the path. Stored figure paths are absolute and can be stale.
    Missing,
    /// The file is there but is not a PNG or JPEG that can be decoded.
    Unreadable,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImageState {
    Loading,
    Ready(Picture),
    Failed(ImageFailure),
}

/// A loaded picture. Valid only in the frame that returned it: do not store it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Picture {
    size: egui::Vec2,
    levels: [Option<(egui::TextureId, u32)>; MAX_LEVELS],
}

impl Picture {
    /// `levels` holds each stored texture with its width in pixels, the widest first.
    pub(super) fn new(
        size: egui::Vec2,
        levels: impl IntoIterator<Item = (egui::TextureId, u32)>,
    ) -> Picture {
        let mut stored = [None; MAX_LEVELS];
        for (slot, level) in stored.iter_mut().zip(levels) {
            *slot = Some(level);
        }
        Picture {
            size,
            levels: stored,
        }
    }

    /// Width and height of the file's picture, in its own pixels.
    pub fn size(&self) -> egui::Vec2 {
        self.size
    }

    /// The stored size to use when the whole picture is drawn `shown_width` points wide: the
    /// smallest one that has a pixel for every pixel of the screen, or the widest when none has.
    pub fn texture(&self, shown_width: f32, pixels_per_point: f32) -> egui::TextureId {
        let needed = shown_width * pixels_per_point;
        // Every picture has at least one stored size, so the default texture is never the answer.
        self.levels
            .iter()
            .flatten()
            .rev()
            .find(|(_, width)| *width as f32 >= needed)
            .or(self.levels[0].as_ref())
            .map_or_else(egui::TextureId::default, |(texture, _)| *texture)
    }

    /// Paints the whole picture into `rect` with the right stored size. For zoom and pan, pass
    /// the full picture rectangle, even where it lies off the screen, and let the clip rectangle
    /// cut it.
    pub fn paint(&self, painter: &egui::Painter, rect: egui::Rect, tint: egui::Color32) {
        let texture = self.texture(rect.width(), painter.pixels_per_point());
        let whole = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        painter.image(texture, rect, whole, tint);
    }
}
