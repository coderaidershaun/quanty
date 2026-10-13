//! The colours, sizes, text styles, fonts and icons that every panel draws with, and the style
//! that makes stock egui widgets match them. Together they make the app look like paper and ink:
//! a pale desk, sheets of paper, a darker well under the scanned page, Literata for what is read,
//! Source Sans 3 for controls, and the rubric red only for the thread from a citation to its
//! source.

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
pub use metrics::{hairline, motion, radius, shadow, size, space, stroke};
pub use palette::{Swatch, Tone, color};
pub use typography::TextRole;

/// Sets the fonts and the style of the whole app. Call it once, in the closure that makes the
/// eframe app, because fonts that are set while a frame is drawn only exist from the next frame.
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(fonts::definitions());
    ctx.set_theme(egui::Theme::Light);
    ctx.all_styles_mut(style::apply);
}

/// The colour of the window behind the panels, in the form `eframe::App::clear_color` returns.
pub fn clear_color() -> [f32; 4] {
    color::DESK.to_normalized_gamma_f32()
}
