//! The colours, sizes, text styles, fonts and icons that every panel draws with, and the style
//! that makes stock egui widgets match them. Together they make the app look like paper and ink:
//! a desk, sheets of paper, a darker well under the scanned page, Literata for what is read,
//! Source Sans 3 for controls, and the rubric red only for the thread from a citation to its
//! source. The colours come in two palettes, dark and light, and the window draws with one.

mod fonts;
mod icons;
mod kind;
mod metrics;
mod palette;
mod scheme;
mod style;
mod typography;

use eframe::egui;

pub use icons::Icon;
pub use kind::Kind;
pub use metrics::{hairline, motion, radius, shadow, size, space, stroke};
pub use palette::{Palette, Swatch, Tone};
pub use scheme::Scheme;
pub use typography::TextRole;

/// The tint that draws a picture with its own colours. A scanned page and a figure keep their look
/// in both schemes, so this is not a colour of a palette.
pub const UNTINTED: egui::Color32 = egui::Color32::WHITE;

/// Sets the fonts and the style of both schemes, and opens the window in `Scheme::default()`. Call
/// it once, in the closure that makes the eframe app, because fonts that are set while a frame is
/// drawn only exist from the next frame.
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(fonts::definitions());
    for scheme in Scheme::ALL {
        ctx.style_mut_of(scheme.into(), |style| style::apply(style, scheme));
    }
    // The page around the canvas takes its colour from the host, so egui must not also ask the
    // window for a theme each time the scheme changes.
    ctx.options_mut(|options| options.sync_window_theme = false);
    set_scheme(ctx, Scheme::default());
}

/// The scheme the window is drawn with now.
pub fn scheme(ctx: &egui::Context) -> Scheme {
    ctx.theme().into()
}

/// Egui gives a pass the style of the scheme at its start, so call this only between passes or at
/// the end of one. A change in the middle of a pass would draw one frame in two palettes.
pub fn set_scheme(ctx: &egui::Context, scheme: Scheme) {
    ctx.set_theme(egui::Theme::from(scheme));
}

/// The colours the window is drawn with now.
pub fn palette(ctx: &egui::Context) -> &'static Palette {
    scheme(ctx).palette()
}
