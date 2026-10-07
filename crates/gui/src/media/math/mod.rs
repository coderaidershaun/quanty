//! Formulas as pictures. Nothing draws a formula yet: every one fails, and the LaTeX source is
//! shown in monospace, which is also what a formula that cannot be drawn will show.

use std::collections::HashMap;

use eframe::egui;

use super::{Media, Offload};
use crate::theme::{TextRole, color};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Display {
    Inline,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MathRef<'a> {
    pub latex: &'a str,
    pub display: Display,
    /// The em of the formula, in points.
    pub size: f32,
}

impl<'a> MathRef<'a> {
    pub fn block(latex: &'a str) -> Self {
        MathRef {
            latex,
            display: Display::Block,
            size: crate::theme::size::MATH_DISPLAY,
        }
    }

    /// A formula set in a line of text in this role.
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
    Failed,
}

/// A drawn formula. The measures are in points; the ink is white.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MathImage {
    pub texture: egui::TextureId,
    pub texture_size: egui::Vec2,
    pub bleed: f32,
    pub width: f32,
    pub ascent: f32,
    pub descent: f32,
}

impl MathImage {
    pub fn size(&self) -> egui::Vec2 {
        egui::vec2(self.width, self.ascent + self.descent)
    }

    /// Paints the formula with its baseline starting at `baseline_left`.
    pub fn paint(
        &self,
        painter: &egui::Painter,
        baseline_left: egui::Pos2,
        scale: f32,
        tint: egui::Color32,
    ) {
        let top_left = baseline_left - egui::vec2(0.0, self.ascent * scale);
        let rect = egui::Rect::from_min_size(top_left, self.size() * scale);
        let whole = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        painter.image(self.texture, rect, whole, tint);
    }
}

type Key = (String, Display, u32);

pub struct Math {
    ctx: egui::Context,
    offload: Offload,
    entries: HashMap<Key, MathState>,
    queue: Vec<Key>,
}

impl Math {
    pub fn new(ctx: &egui::Context, offload: Offload) -> Math {
        Math {
            ctx: ctx.clone(),
            offload,
            entries: HashMap::new(),
            queue: Vec::new(),
        }
    }

    /// Limits the bytes of textures kept. It has no effect yet.
    pub fn with_budget(self, _bytes: usize) -> Math {
        self
    }

    pub fn poll(&mut self, _ctx: &egui::Context) {
        if self.offload == Offload::Threads {
            self.run_pending();
        }
    }

    /// Manual only: finishes every queued formula here.
    pub fn run_pending(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        for key in std::mem::take(&mut self.queue) {
            self.entries.insert(key, MathState::Failed);
        }
        self.ctx.request_repaint();
    }

    /// True when no formula is loading.
    pub fn is_idle(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn get(&mut self, math: &MathRef<'_>) -> MathState {
        let key = (math.latex.to_owned(), math.display, math.size.to_bits());
        if let Some(state) = self.entries.get(&key) {
            return *state;
        }
        self.entries.insert(key.clone(), MathState::Loading);
        self.queue.push(key);
        MathState::Loading
    }
}

/// Shows a formula. While it loads, and when it cannot be drawn, the LaTeX source is shown in
/// monospace as one node named by the source.
pub fn show(ui: &mut egui::Ui, media: &mut Media, math: &MathRef<'_>) -> egui::Response {
    match media.math.get(math) {
        MathState::Ready(image) => {
            let (rect, response) = ui.allocate_exact_size(image.size(), egui::Sense::hover());
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Image, true, math.latex)
            });
            image.paint(
                ui.painter(),
                rect.left_top() + egui::vec2(0.0, image.ascent),
                1.0,
                color::TEXT,
            );
            response
        }
        MathState::Loading | MathState::Failed => ui.label(TextRole::Mono.rich(math.latex)),
    }
}
