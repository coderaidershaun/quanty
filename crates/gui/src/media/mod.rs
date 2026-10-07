//! Pictures, formulas and laid-out text that panels show: each is made off the window's thread
//! the first time it is asked for, and kept.

pub mod images;
pub mod math;
mod offload;
pub mod rich_text;

use eframe::egui;

pub use offload::{Offload, Urgency, Workers};

/// Everything a panel may ask for that takes time to make.
pub struct Media {
    pub images: images::Images,
    pub math: math::Math,
    pub text: rich_text::Layouts,
}

impl Media {
    /// For the app: work runs on worker threads.
    pub fn new(ctx: &egui::Context) -> Media {
        Media::with_offload(ctx, Offload::Threads)
    }

    /// For tests: nothing runs until `run_pending`.
    pub fn manual(ctx: &egui::Context) -> Media {
        Media::with_offload(ctx, Offload::Manual)
    }

    fn with_offload(ctx: &egui::Context, offload: Offload) -> Media {
        Media {
            images: images::Images::new(ctx, offload),
            math: math::Math::new(ctx, offload),
            text: rich_text::Layouts::new(),
        }
    }

    /// Takes in what the workers finished. The app calls it once for every frame.
    pub fn poll(&mut self, ctx: &egui::Context) {
        self.images.poll(ctx);
        self.math.poll(ctx);
        self.text.poll(ctx);
    }

    /// Manual only: does every queued job here.
    pub fn run_pending(&mut self) {
        self.images.run_pending();
        self.math.run_pending();
    }

    /// True when no picture and no formula is loading.
    pub fn is_idle(&self) -> bool {
        self.images.is_idle() && self.math.is_idle()
    }
}
