//! The colours, sizes, text styles, fonts and icons that every panel draws with, and the style
//! that makes stock egui widgets match them.

mod fonts;
mod icons;
mod kind;
mod metrics;
mod palette;
mod style;
mod typography;

use eframe::egui;

pub use icons::Icon;
pub use kind::Kind;
pub use metrics::{hairline, motion, radius, size, space, stroke};
pub use palette::{Swatch, Tone, color, glow};
pub use typography::TextRole;

/// Fonts and style. Call it once, before the first pass: the eframe creation closure.
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(fonts::definitions());
    ctx.set_theme(egui::Theme::Dark);
    ctx.all_styles_mut(style::apply);
}

/// `color::CANVAS`, for `eframe::App::clear_color`.
pub fn clear_color() -> [f32; 4] {
    color::CANVAS.to_normalized_gamma_f32()
}
