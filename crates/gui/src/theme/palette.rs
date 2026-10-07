//! The colours of the app: the named surfaces and text colours, and the seven tones, each with the
//! five colours that a chip, a button or a notice of that tone is made of.

use eframe::egui::{self, Color32};

/// A soft halo in the tone's solid colour: `painter.add(glow(tone).as_shape(rect, radius))`.
pub fn glow(tone: Tone) -> egui::Shadow {
    egui::Shadow {
        offset: [0, 0],
        blur: 16,
        spread: 0,
        color: tone.swatch().solid.gamma_multiply(0.35),
    }
}

pub mod color {
    use super::Color32;

    pub const CANVAS: Color32 = Color32::from_rgb(0x07, 0x0E, 0x1A);
    pub const PANEL: Color32 = Color32::from_rgb(0x0A, 0x14, 0x21);
    pub const RAISED: Color32 = Color32::from_rgb(0x0E, 0x1C, 0x31);
    pub const RAISED_HOVER: Color32 = Color32::from_rgb(0x14, 0x27, 0x3F);
    pub const BORDER: Color32 = Color32::from_rgb(0x22, 0x3B, 0x5C);
    pub const HAIRLINE: Color32 = Color32::from_rgb(0x14, 0x26, 0x3C);
    pub const TEXT: Color32 = Color32::from_rgb(0xEE, 0xF4, 0xFB);
    pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(0xB4, 0xC3, 0xD6);
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x84, 0x96, 0xB0);
    pub const TEXT_ON_ACCENT: Color32 = Color32::from_rgb(0x0A, 0x06, 0x12);
    pub const PAGE: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    pub const FOCUS: Color32 = Color32::from_rgb(0x7D, 0xB9, 0xFF);
    pub const SELECTION: Color32 = Color32::from_rgba_unmultiplied_const(0x0B, 0x7B, 0xF5, 0x66);
    pub const SCRIM: Color32 = Color32::from_rgba_unmultiplied_const(0x07, 0x0E, 0x1A, 0xB8);
    pub const PAGE_HIGHLIGHT: Color32 =
        Color32::from_rgba_unmultiplied_const(0x0B, 0x7B, 0xF5, 0x38);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tone {
    Neutral,
    Blue,
    Purple,
    Magenta,
    Success,
    Warning,
    Danger,
}

/// The five colours of one tone. `solid` is a fill or a stroke, `on_solid` is text on that
/// fill, `text` is the hue as readable text on a dark surface, `wash` is a dark tinted fill,
/// and `edge` is the outline of that fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Swatch {
    pub solid: Color32,
    pub on_solid: Color32,
    pub text: Color32,
    pub wash: Color32,
    pub edge: Color32,
}

const fn swatch(solid: u32, on_solid: u32, text: u32, wash: u32, edge: u32) -> Swatch {
    const fn rgb(hex: u32) -> Color32 {
        Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
    }
    Swatch {
        solid: rgb(solid),
        on_solid: rgb(on_solid),
        text: rgb(text),
        wash: rgb(wash),
        edge: rgb(edge),
    }
}

impl Tone {
    /// Every tone, in the order of the enum.
    pub(crate) const ALL: [Tone; 7] = [
        Tone::Neutral,
        Tone::Blue,
        Tone::Purple,
        Tone::Magenta,
        Tone::Success,
        Tone::Warning,
        Tone::Danger,
    ];

    /// The five colours of this tone.
    pub const fn swatch(self) -> Swatch {
        match self {
            Tone::Neutral => swatch(0x8496B0, 0x0A0612, 0xB4C3D6, 0x0E1C31, 0x223B5C),
            Tone::Blue => swatch(0x0B7BF5, 0x0A0612, 0x4DA6FF, 0x05245A, 0x0F58B8),
            Tone::Purple => swatch(0x7A30F4, 0xEEF4FB, 0xA98BFF, 0x1D0C48, 0x6A2AD0),
            Tone::Magenta => swatch(0xF800C4, 0x0A0612, 0xFF4FD8, 0x3A053A, 0xC0089A),
            Tone::Success => swatch(0x22C58B, 0x0A0612, 0x4ADEA5, 0x062B22, 0x127A58),
            Tone::Warning => swatch(0xF2A93B, 0x0A0612, 0xF7C266, 0x33230A, 0x8F6217),
            Tone::Danger => swatch(0xF0503C, 0x0A0612, 0xFF8070, 0x3A120E, 0x9E2E22),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SURFACES: [(&str, Color32); 4] = [
        ("CANVAS", color::CANVAS),
        ("PANEL", color::PANEL),
        ("RAISED", color::RAISED),
        ("RAISED_HOVER", color::RAISED_HOVER),
    ];

    const TEXTS: [(&str, Color32); 3] = [
        ("TEXT", color::TEXT),
        ("TEXT_SECONDARY", color::TEXT_SECONDARY),
        ("TEXT_MUTED", color::TEXT_MUTED),
    ];

    #[test]
    fn text_tokens_meet_contrast_on_every_surface() {
        let mut misses = Vec::new();
        let mut require = |what: String, foreground: Color32, background: Color32, least: f32| {
            let ratio = contrast(foreground, background);
            if ratio < least {
                misses.push(format!("{what}: {ratio:.2}, needs {least}"));
            }
        };
        for (text_name, text) in TEXTS {
            for (surface_name, surface) in SURFACES {
                require(format!("{text_name} on {surface_name}"), text, surface, 4.5);
            }
        }
        for tone in Tone::ALL {
            let swatch = tone.swatch();
            require(
                format!("{tone:?} on_solid on solid"),
                swatch.on_solid,
                swatch.solid,
                4.5,
            );
            require(
                format!("{tone:?} text on PANEL"),
                swatch.text,
                color::PANEL,
                4.5,
            );
            require(
                format!("{tone:?} text on RAISED"),
                swatch.text,
                color::RAISED,
                4.5,
            );
            require(
                format!("{tone:?} text on wash"),
                swatch.text,
                swatch.wash,
                4.5,
            );
            require(
                format!("TEXT on {tone:?} wash"),
                color::TEXT,
                swatch.wash,
                13.0,
            );
            require(
                format!("{tone:?} solid on PANEL"),
                swatch.solid,
                color::PANEL,
                3.0,
            );
        }
        assert!(misses.is_empty(), "text that is hard to read: {misses:#?}");
    }

    /// WCAG 2 contrast ratio of two opaque colours.
    fn contrast(first: Color32, second: Color32) -> f32 {
        let (light, dark) = (luminance(first), luminance(second));
        let (light, dark) = if light >= dark {
            (light, dark)
        } else {
            (dark, light)
        };
        (light + 0.05) / (dark + 0.05)
    }

    fn luminance(color: Color32) -> f32 {
        let linear = |channel: u8| {
            let value = f32::from(channel) / 255.0;
            if value <= 0.03928 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
    }
}
