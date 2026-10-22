//! Pictures, formulas and laid-out text are prepared once and kept while panels need them.

pub mod code;
mod fetch;
pub mod images;
pub mod math;
mod offload;
pub mod rich_text;
mod salt;

use std::time::Duration;

use eframe::egui;
use web_time::Instant;

pub use fetch::{Fetch, UnavailableFetch};

pub use offload::{Offload, Urgency, Workers};
use salt::content_salt;

pub struct Media {
    pub images: images::Images,
    pub math: math::Math,
    pub text: rich_text::Layouts,
    automatic: bool,
}

impl Media {
    pub fn new(ctx: &egui::Context) -> Media {
        Media::with_fetch(ctx, Box::new(UnavailableFetch))
    }

    pub fn with_fetch(ctx: &egui::Context, fetch: Box<dyn Fetch>) -> Media {
        let mut media = Media::manual_with_fetch(ctx, fetch);
        media.automatic = true;
        media
    }

    /// For tests: nothing runs until `run_pending`.
    pub fn manual(ctx: &egui::Context) -> Media {
        Media::manual_with_fetch(ctx, Box::new(UnavailableFetch))
    }

    pub fn manual_with_fetch(ctx: &egui::Context, fetch: Box<dyn Fetch>) -> Media {
        Media {
            images: images::Images::new(ctx, Offload::Manual, fetch),
            math: math::Math::new(ctx, Offload::Manual),
            text: rich_text::Layouts::default(),
            automatic: false,
        }
    }

    /// The app calls it once for every frame.
    pub fn poll(&mut self, ctx: &egui::Context) {
        if self.automatic {
            let started = Instant::now();
            let budget = Duration::from_millis(4);
            self.images.run_for(budget);
            self.math.run_for(budget.saturating_sub(started.elapsed()));
        }
        self.images.poll(ctx);
        self.math.poll(ctx);
        self.text.poll(ctx);
    }

    /// Manual only: does every queued job here.
    pub fn run_pending(&mut self) {
        self.images.run_pending();
        self.math.run_pending();
    }

    pub fn is_idle(&self) -> bool {
        self.images.is_idle() && self.math.is_idle()
    }
}
