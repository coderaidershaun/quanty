//! Pictures from disk as textures: asked for by path, decoded once, shown or marked failed.

use std::collections::HashMap;

use eframe::egui;

use super::{Media, Offload};
use crate::contract::ImageRef;
use crate::widgets::{Placeholder, spinner};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFailure {
    Missing,
    Unreadable,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImageState {
    Loading,
    Ready(Picture),
    Failed(ImageFailure),
}

/// A picture that is ready to paint. It is valid for the frame it was asked for in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Picture {
    texture: egui::TextureId,
    size: egui::Vec2,
}

impl Picture {
    /// The file's own pixels.
    pub fn size(&self) -> egui::Vec2 {
        self.size
    }

    /// The texture to paint when the picture is shown `shown_width` points wide.
    pub fn texture(&self, _shown_width: f32, _pixels_per_point: f32) -> egui::TextureId {
        self.texture
    }

    pub fn paint(&self, painter: &egui::Painter, rect: egui::Rect, tint: egui::Color32) {
        let whole = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        painter.image(self.texture, rect, whole, tint);
    }
}

enum Entry {
    Loading,
    Ready {
        texture: egui::TextureHandle,
        size: egui::Vec2,
    },
    Failed(ImageFailure),
}

pub struct Images {
    ctx: egui::Context,
    offload: Offload,
    entries: HashMap<ImageRef, Entry>,
    queue: Vec<ImageRef>,
}

impl Images {
    pub fn new(ctx: &egui::Context, offload: Offload) -> Images {
        Images {
            ctx: ctx.clone(),
            offload,
            entries: HashMap::new(),
            queue: Vec::new(),
        }
    }

    /// Limits the bytes of textures kept. It has no effect yet.
    pub fn with_budget(self, _bytes: usize) -> Images {
        self
    }

    /// Takes in what finished. In the app this decodes here, on the window's thread.
    pub fn poll(&mut self, _ctx: &egui::Context) {
        if self.offload == Offload::Threads {
            self.run_pending();
        }
    }

    /// Manual only: decodes every queued picture here.
    pub fn run_pending(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        for image in std::mem::take(&mut self.queue) {
            let entry = decode(&self.ctx, &image);
            self.entries.insert(image, entry);
        }
        self.ctx.request_repaint();
    }

    /// True when no picture is loading.
    pub fn is_idle(&self) -> bool {
        self.queue.is_empty()
    }

    /// The picture, or what it is doing. A picture asked for the first time starts loading.
    pub fn get(&mut self, image: &ImageRef) -> ImageState {
        match self.entries.get(image) {
            Some(Entry::Ready { texture, size }) => ImageState::Ready(Picture {
                texture: texture.id(),
                size: *size,
            }),
            Some(Entry::Failed(failure)) => ImageState::Failed(*failure),
            Some(Entry::Loading) => ImageState::Loading,
            None => {
                self.entries.insert(image.clone(), Entry::Loading);
                self.queue.push(image.clone());
                ImageState::Loading
            }
        }
    }

    /// Starts a picture that is likely to be asked for next, such as a neighbouring page.
    pub fn prefetch(&mut self, image: &ImageRef) {
        self.get(image);
    }

    /// Forgets a failure, so the next `get` tries again.
    pub fn retry(&mut self, image: &ImageRef) {
        if matches!(self.entries.get(image), Some(Entry::Failed(_))) {
            self.entries.remove(image);
        }
    }
}

fn decode(ctx: &egui::Context, image: &ImageRef) -> Entry {
    if !image.path.exists() {
        return Entry::Failed(ImageFailure::Missing);
    }
    let Ok(decoded) = image::open(&image.path) else {
        return Entry::Failed(ImageFailure::Unreadable);
    };
    let rgba = decoded.into_rgba8();
    let pixels = [rgba.width() as usize, rgba.height() as usize];
    let color_image = egui::ColorImage::from_rgba_unmultiplied(pixels, rgba.as_raw());
    let size = egui::vec2(rgba.width() as f32, rgba.height() as f32);
    let name = image.path.display().to_string();
    Entry::Ready {
        texture: ctx.load_texture(name, color_image, egui::TextureOptions::LINEAR),
        size,
    }
}

/// Shows a picture no larger than `max_size`: a spinner while it loads, a placeholder when it
/// failed. `alt` is its accessible name.
pub fn show(
    ui: &mut egui::Ui,
    media: &mut Media,
    image: &ImageRef,
    alt: &str,
    max_size: egui::Vec2,
) -> egui::Response {
    match media.images.get(image) {
        ImageState::Loading => spinner(ui, alt),
        ImageState::Failed(_) => Placeholder::error(alt).show(ui).response,
        ImageState::Ready(picture) => {
            let own = picture.size();
            let scale = (max_size.x / own.x).min(max_size.y / own.y).min(1.0);
            let (rect, response) = ui.allocate_exact_size(own * scale, egui::Sense::hover());
            response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Image, true, alt));
            if ui.is_rect_visible(rect) {
                picture.paint(ui.painter(), rect, egui::Color32::WHITE);
            }
            response
        }
    }
}
