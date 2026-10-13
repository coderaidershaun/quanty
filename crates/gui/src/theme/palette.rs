//! The colours of the app: the named surfaces, inks and text colours of the paper, and the ten
//! tones, each with the five colours that a chip, a button or a notice of that tone is made of.

use eframe::egui::Color32;

/// `DESK` is the window and the panels drawn straight on it, `SHEET` a sheet of paper on the
/// desk, and `WELL` a bed or an input sunk into a sheet, `WELL_DEEP` when it is pressed. `BORDER`
/// outlines a control and `HAIRLINE` rules a sheet. Content is `TEXT`, titles `TEXT_HEADING`,
/// text that supports content `TEXT_SECONDARY`, and a placeholder or a zero count `TEXT_MUTED`.
pub mod color {
    use super::Color32;

    pub const DESK: Color32 = Color32::from_rgb(0xF2, 0xF0, 0xE5);
    pub const SHEET: Color32 = Color32::from_rgb(0xFF, 0xFC, 0xF0);
    pub const WELL: Color32 = Color32::from_rgb(0xE6, 0xE4, 0xD9);
    pub const WELL_DEEP: Color32 = Color32::from_rgb(0xDA, 0xD8, 0xCE);
    pub const HAIRLINE: Color32 = Color32::from_rgb(0xDA, 0xD8, 0xCE);
    pub const BORDER: Color32 = Color32::from_rgb(0x87, 0x85, 0x80);
    pub const RULE: Color32 = Color32::from_rgb(0x1C, 0x1B, 0x1A);
    pub const TEXT: Color32 = Color32::from_rgb(0x1C, 0x1B, 0x1A);
    pub const TEXT_HEADING: Color32 = Color32::from_rgb(0x10, 0x0F, 0x0F);
    pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(0x57, 0x56, 0x53);
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x6F, 0x6E, 0x69);
    pub const TEXT_FAINT: Color32 = Color32::from_rgb(0xB7, 0xB5, 0xAC);
    pub const TEXT_ON_INK: Color32 = Color32::from_rgb(0xFF, 0xFC, 0xF0);
    pub const INK: Color32 = Color32::from_rgb(0x1C, 0x1B, 0x1A);
    pub const INK_HOVER: Color32 = Color32::from_rgb(0x40, 0x3E, 0x3C);
    pub const INK_PRESSED: Color32 = Color32::from_rgb(0x10, 0x0F, 0x0F);
    pub const PAGE: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    pub const FOCUS: Color32 = Color32::from_rgb(0x20, 0x5E, 0xA6);
    pub const SELECTION: Color32 = Color32::from_rgb(0xC6, 0xDD, 0xE8);
    pub const SCRIM: Color32 = Color32::from_rgba_unmultiplied_const(0x1C, 0x1B, 0x1A, 0x59);
    pub const PAGE_HIGHLIGHT: Color32 =
        Color32::from_rgba_unmultiplied_const(0xAF, 0x30, 0x29, 0x24);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tone {
    Ink,
    Rubric,
    Indigo,
    Purple,
    Verdigris,
    Ochre,
    Madder,
    Success,
    Warning,
    Danger,
}

/// `solid` is a fill or a stroke, `on_solid` is text on that fill, `text` is the hue as readable
/// text on paper, `wash` is a pale tinted fill, and `edge` is the outline of that fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Swatch {
    pub solid: Color32,
    pub on_solid: Color32,
    pub text: Color32,
    pub wash: Color32,
    pub edge: Color32,
}

const fn swatch(solid: u32, text: u32, wash: u32, edge: u32) -> Swatch {
    const fn rgb(hex: u32) -> Color32 {
        Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
    }
    Swatch {
        solid: rgb(solid),
        on_solid: color::TEXT_ON_INK,
        text: rgb(text),
        wash: rgb(wash),
        edge: rgb(edge),
    }
}

impl Tone {
    pub(crate) const ALL: [Tone; 10] = [
        Tone::Ink,
        Tone::Rubric,
        Tone::Indigo,
        Tone::Purple,
        Tone::Verdigris,
        Tone::Ochre,
        Tone::Madder,
        Tone::Success,
        Tone::Warning,
        Tone::Danger,
    ];

    pub const fn swatch(self) -> Swatch {
        match self {
            Tone::Ink => swatch(0x575653, 0x403E3C, 0xE6E4D9, 0x9F9D96),
            Tone::Rubric => swatch(0xAF3029, 0x942822, 0xFFE1D5, 0xD14D41),
            Tone::Indigo => swatch(0x205EA6, 0x1A4F8C, 0xE1ECEB, 0x4385BE),
            Tone::Purple => swatch(0x5E409D, 0x4F3685, 0xF0EAEC, 0x8B7EC8),
            Tone::Verdigris => swatch(0x24837B, 0x1C6C66, 0xDDF1E4, 0x3AA99F),
            Tone::Ochre => swatch(0xAD8301, 0x8E6B01, 0xFAEEC6, 0xD0A215),
            Tone::Madder => swatch(0xA02F6F, 0x87285E, 0xFEE4E5, 0xCE5D97),
            Tone::Success => swatch(0x66800B, 0x536907, 0xEDEECF, 0x879A39),
            Tone::Warning => swatch(0xBC5215, 0x9D4310, 0xFFE7CE, 0xDA702C),
            Tone::Danger => swatch(0xAF3029, 0x942822, 0xFFE1D5, 0xD14D41),
        }
    }
}
