//! The colours of the app in both palettes, light and dark: the named surfaces, inks and text
//! colours of the paper, and the ten tones, each with the five colours that a chip, a button or a
//! notice of that tone is made of.

use eframe::egui::Color32;

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

impl Tone {
    pub const ALL: [Tone; 10] = [
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
}

/// Every colour the app draws with, for one scheme. The tones are kept in the order of
/// `Tone::ALL`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// The window and the panels drawn straight on it.
    pub desk: Color32,
    /// A sheet of paper on the desk.
    pub sheet: Color32,
    /// A bed or an input sunk into a sheet.
    pub well: Color32,
    /// A well when it is pressed.
    pub well_deep: Color32,
    /// Rules a sheet.
    pub hairline: Color32,
    /// Outlines a control.
    pub border: Color32,
    /// A heavy line, and the outline of what is selected.
    pub rule: Color32,
    /// Content.
    pub text: Color32,
    /// Titles.
    pub text_heading: Color32,
    /// Text that supports content.
    pub text_secondary: Color32,
    /// A placeholder or a zero count.
    pub text_muted: Color32,
    /// Only for a mark that has words beside it.
    pub text_faint: Color32,
    /// Text on a fill of `ink`.
    pub text_on_ink: Color32,
    /// The fill of the main button.
    pub ink: Color32,
    pub ink_hover: Color32,
    pub ink_pressed: Color32,
    /// The paper of a scanned page, which keeps its own look in both palettes.
    pub page: Color32,
    /// The ring of the control that has the keyboard.
    pub focus: Color32,
    /// The fill behind selected text.
    pub selection: Color32,
    /// What dims the window behind a sheet that asks a question.
    pub scrim: Color32,
    /// What marks the cited figure on a scanned page.
    pub page_highlight: Color32,
    /// The colour of the one shadow, which a sheet of paper casts.
    pub shadow: Color32,
    tones: [Swatch; 10],
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// The alpha is not multiplied in, as the colour is written.
const fn rgba(hex: u32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied_const((hex >> 16) as u8, (hex >> 8) as u8, hex as u8, alpha)
}

const fn swatch(on_solid: Color32, [solid, text, wash, edge]: [u32; 4]) -> Swatch {
    Swatch {
        solid: rgb(solid),
        on_solid,
        text: rgb(text),
        wash: rgb(wash),
        edge: rgb(edge),
    }
}

/// Each row is `[solid, text, wash, edge]`, in the order of `Tone::ALL`.
const fn tones(on_solid: Color32, rows: [[u32; 4]; 10]) -> [Swatch; 10] {
    let mut tones = [swatch(on_solid, rows[0]); 10];
    let mut index = 1;
    while index < tones.len() {
        tones[index] = swatch(on_solid, rows[index]);
        index += 1;
    }
    tones
}

impl Palette {
    pub const LIGHT: Palette = {
        let text_on_ink = rgb(0xFFFCF0);
        Palette {
            desk: rgb(0xF2F0E5),
            sheet: rgb(0xFFFCF0),
            well: rgb(0xE6E4D9),
            well_deep: rgb(0xDAD8CE),
            hairline: rgb(0xDAD8CE),
            border: rgb(0x878580),
            rule: rgb(0x1C1B1A),
            text: rgb(0x1C1B1A),
            text_heading: rgb(0x100F0F),
            text_secondary: rgb(0x575653),
            text_muted: rgb(0x676662),
            text_faint: rgb(0xB7B5AC),
            text_on_ink,
            ink: rgb(0x1C1B1A),
            ink_hover: rgb(0x403E3C),
            ink_pressed: rgb(0x100F0F),
            page: rgb(0xFFFFFF),
            focus: rgb(0x205EA6),
            selection: rgb(0xC6DDE8),
            scrim: rgba(0x1C1B1A, 0x59),
            page_highlight: rgba(0xAF3029, 0x14),
            shadow: rgba(0x100F0F, 0x1F),
            tones: tones(
                text_on_ink,
                [
                    [0x575653, 0x403E3C, 0xE6E4D9, 0x9F9D96],
                    [0xAF3029, 0x942822, 0xFFE1D5, 0xD14D41],
                    [0x205EA6, 0x1A4F8C, 0xE1ECEB, 0x4385BE],
                    [0x5E409D, 0x4F3685, 0xF0EAEC, 0x8B7EC8],
                    [0x24837B, 0x1C6C66, 0xDDF1E4, 0x3AA99F],
                    [0xAD8301, 0x816100, 0xFAEEC6, 0xD0A215],
                    [0xA02F6F, 0x87285E, 0xFEE4E5, 0xCE5D97],
                    [0x66800B, 0x536907, 0xEDEECF, 0x879A39],
                    [0xBC5215, 0x9D4310, 0xFFE7CE, 0xDA702C],
                    [0xAF3029, 0x942822, 0xFFE1D5, 0xD14D41],
                ],
            ),
        }
    };

    pub const DARK: Palette = {
        let text_on_ink = rgb(0x100F0F);
        Palette {
            desk: rgb(0x100F0F),
            sheet: rgb(0x1C1B1A),
            well: rgb(0x282726),
            well_deep: rgb(0x343331),
            hairline: rgb(0x343331),
            border: rgb(0x757470),
            rule: rgb(0xCECDC3),
            text: rgb(0xCECDC3),
            text_heading: rgb(0xE6E4D9),
            text_secondary: rgb(0xA8A69F),
            text_muted: rgb(0x95938C),
            text_faint: rgb(0x575653),
            text_on_ink,
            ink: rgb(0xCECDC3),
            ink_hover: rgb(0xB7B5AC),
            ink_pressed: rgb(0xE6E4D9),
            page: rgb(0xFFFFFF),
            focus: rgb(0x4385BE),
            selection: rgb(0x163B66),
            scrim: rgba(0x100F0F, 0x99),
            page_highlight: rgba(0xD4574A, 0x1F),
            shadow: rgba(0x000000, 0x66),
            tones: tones(
                text_on_ink,
                [
                    [0xB7B5AC, 0xCECDC3, 0x282726, 0x6F6E69],
                    [0xD4574A, 0xE8705F, 0x3E1715, 0xAF3029],
                    [0x4385BE, 0x66A0C8, 0x12253B, 0x205EA6],
                    [0x8B7EC8, 0xA699D0, 0x261C39, 0x5E409D],
                    [0x3AA99F, 0x5ABDAC, 0x122F2C, 0x24837B],
                    [0xD0A215, 0xDFB431, 0x3A2D04, 0xAD8301],
                    [0xCE5D97, 0xE47DA8, 0x39172B, 0xA02F6F],
                    [0x879A39, 0xA0AF54, 0x252D09, 0x66800B],
                    [0xDA702C, 0xEC8B49, 0x40200D, 0xBC5215],
                    [0xD4574A, 0xE8705F, 0x3E1715, 0xAF3029],
                ],
            ),
        }
    };

    pub const fn tone(&self, tone: Tone) -> Swatch {
        self.tones[tone as usize]
    }
}
