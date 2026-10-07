//! The sizes of the app, in points: spacing, corner radii, control and icon sizes, line widths and
//! the two animation times. The one-pixel line is here too.

use eframe::egui::{self, Color32, Stroke};

/// A stroke one physical pixel wide.
pub fn hairline(painter: &egui::Painter, color: Color32) -> Stroke {
    Stroke::new(1.0 / painter.ctx().pixels_per_point(), color)
}

/// Points.
pub mod space {
    pub const XXS: f32 = 2.0;
    pub const XS: f32 = 4.0;
    pub const SM: f32 = 8.0;
    pub const MD: f32 = 12.0;
    pub const LG: f32 = 16.0;
    pub const XL: f32 = 24.0;
    pub const XXL: f32 = 32.0;
}

/// Points.
pub mod radius {
    pub const SM: f32 = 4.0;
    pub const MD: f32 = 6.0;
    pub const LG: f32 = 8.0;
    pub const XL: f32 = 12.0;
}

/// Points.
pub mod size {
    pub const CONTROL_SM: f32 = 28.0;
    pub const CONTROL_MD: f32 = 32.0;
    pub const CONTROL_LG: f32 = 40.0;
    pub const ICON_SM: f32 = 14.0;
    pub const ICON_MD: f32 = 16.0;
    pub const ICON_LG: f32 = 20.0;
    pub const CHIP: f32 = 24.0;
    pub const BADGE: f32 = 20.0;
    pub const CITATION: f32 = 18.0;
    pub const STEP: f32 = 24.0;
    pub const DOT: f32 = 8.0;
    pub const TAB: f32 = 44.0;
    pub const MATH_DISPLAY: f32 = 20.0;
}

/// Points.
pub mod stroke {
    pub const BORDER: f32 = 1.0;
    pub const FOCUS: f32 = 2.0;
    pub const UNDERLINE: f32 = 2.0;
    pub const EDGE: f32 = 1.5;
}

/// Seconds.
pub mod motion {
    pub const FAST: f32 = 0.12;
    pub const SPINNER_STEP: f32 = 0.1;
}
