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
        let text_on_ink = rgb(0x0B141B);
        Palette {
            desk: rgb(0x0B141B),
            sheet: rgb(0x111B23),
            well: rgb(0x17232D),
            well_deep: rgb(0x2B3A46),
            hairline: rgb(0x242D35),
            border: rgb(0x516979),
            rule: rgb(0xE8EDF1),
            text: rgb(0xE8EDF1),
            text_heading: rgb(0xF3F7FA),
            text_secondary: rgb(0xAEBCC5),
            text_muted: rgb(0x8D9BA7),
            text_faint: rgb(0x54626D),
            text_on_ink,
            ink: rgb(0x3D8BFF),
            ink_hover: rgb(0x5C9DFF),
            ink_pressed: rgb(0x1E79FF),
            page: rgb(0xFFFFFF),
            focus: rgb(0x8CC4FF),
            selection: rgb(0x1C375A),
            scrim: rgba(0x0B141B, 0xB8),
            page_highlight: rgba(0xFF5E53, 0x1F),
            shadow: rgba(0x000000, 0x66),
            tones: tones(
                text_on_ink,
                [
                    [0xAEBCC5, 0xE8EDF1, 0x17232D, 0x34444E],
                    [0xFF5E53, 0xFF7B72, 0x372A30, 0x704143],
                    [0x3D8BFF, 0x8CC4FF, 0x182D46, 0x224677],
                    [0xA371F7, 0xD2A8FF, 0x282945, 0x483C74],
                    [0x39C5CF, 0x76E3EA, 0x17363F, 0x205C64],
                    [0xE6BC83, 0xE6BC83, 0x333532, 0x625847],
                    [0xDB61A2, 0xF778BA, 0x312637, 0x5E3653],
                    [0x3DDC97, 0x3DDC97, 0x183A36, 0x22644F],
                    [0xE6BC83, 0xE6BC83, 0x333532, 0x625847],
                    [0xFF7B72, 0xFF7B72, 0x372A30, 0x704143],
                ],
            ),
        }
    };

    pub const fn tone(&self, tone: Tone) -> Swatch {
        self.tones[tone as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GROUND: Color32 = rgb(0x0B141B);
    const SURFACE: Color32 = rgb(0x111B23);
    const SURFACE_2: Color32 = rgb(0x17232D);
    const FG: Color32 = rgb(0xE8EDF1);
    const MUTED: Color32 = rgb(0x8D9BA7);
    const ON_USER: Color32 = rgb(0xF3F7FA);
    const SECONDARY_TEXT: Color32 = rgb(0xAEBCC5);
    const CONTROL_BORDER: Color32 = rgb(0x34444E);
    const SWITCH_TRACK: Color32 = rgb(0x2B3A46);
    const ACCENT: Color32 = rgb(0x3D8BFF);
    const LINK: Color32 = rgb(0x8CC4FF);
    const FOCUS: Color32 = rgb(0x8CC4FF);
    const DANGER: Color32 = rgb(0xFF7B72);
    const OK: Color32 = rgb(0x3DDC97);
    const AMBER: Color32 = rgb(0xE6BC83);

    /// Jet's danger red is 2.5:1 on the white scanned page, and the outline of the cited figure
    /// needs 3:1, so Rubric's solid is the same red a little darker.
    const RUBRIC_SOLID: u32 = 0xFF5E53;

    /// A colour drawn at `alpha` over `SURFACE`, as one opaque colour: how Jet's washes and
    /// outlines look on its sheet.
    fn over_surface(colour: Color32, alpha: f64) -> Color32 {
        let flatten = |over: u8, under: u8| {
            (f64::from(under) + (f64::from(over) - f64::from(under)) * alpha).round() as u8
        };
        Color32::from_rgb(
            flatten(colour.r(), SURFACE.r()),
            flatten(colour.g(), SURFACE.g()),
            flatten(colour.b(), SURFACE.b()),
        )
    }

    /// Every dark colour taken from Jet's style sheet is checked against its Jet colour. Colours
    /// that Jet does not have, such as the solids and texts of Purple, Verdigris and Madder, are
    /// left out, but those tones still make their washes and edges the way Jet does.
    #[test]
    fn the_dark_palette_takes_jets_colours() {
        let dark = Palette::DARK;
        let ink = dark.tone(Tone::Ink);
        let rubric = dark.tone(Tone::Rubric);
        let indigo = dark.tone(Tone::Indigo);
        let purple = dark.tone(Tone::Purple);
        let verdigris = dark.tone(Tone::Verdigris);
        let ochre = dark.tone(Tone::Ochre);
        let madder = dark.tone(Tone::Madder);
        let success = dark.tone(Tone::Success);
        let warning = dark.tone(Tone::Warning);
        let danger = dark.tone(Tone::Danger);
        let line = over_surface(Color32::WHITE, 0.08);

        let rows = [
            ("desk", dark.desk, GROUND),
            ("sheet", dark.sheet, SURFACE),
            ("well", dark.well, SURFACE_2),
            ("well_deep", dark.well_deep, SWITCH_TRACK),
            ("hairline", dark.hairline, line),
            // Jet's control border is 1.7:1 on the sheet, and the outline of a control needs
            // 3:1, so this is the same hue and saturation, lighter.
            ("border", dark.border, rgb(0x516979)),
            ("rule", dark.rule, FG),
            ("text", dark.text, FG),
            ("text_heading", dark.text_heading, ON_USER),
            ("text_secondary", dark.text_secondary, SECONDARY_TEXT),
            ("text_muted", dark.text_muted, MUTED),
            ("text_on_ink", dark.text_on_ink, GROUND),
            ("ink", dark.ink, ACCENT),
            ("focus", dark.focus, FOCUS),
            ("selection", dark.selection, over_surface(ACCENT, 0.25)),
            // Jet's veil is the ground colour at .72 opacity, and .72 of 255 is 184.
            ("scrim", dark.scrim, rgba(0x0B141B, 184)),
            (
                "page_highlight",
                dark.page_highlight,
                rgba(RUBRIC_SOLID, 0x1F),
            ),
            // Jet's shadow is black at .4 opacity, and .4 of 255 is 102.
            ("shadow", dark.shadow, rgba(0x000000, 102)),
            ("Ink solid", ink.solid, SECONDARY_TEXT),
            ("Ink text", ink.text, FG),
            ("Ink wash", ink.wash, SURFACE_2),
            ("Ink edge", ink.edge, CONTROL_BORDER),
            ("Rubric solid", rubric.solid, rgb(RUBRIC_SOLID)),
            ("Rubric text", rubric.text, DANGER),
            ("Rubric wash", rubric.wash, over_surface(DANGER, 0.16)),
            ("Rubric edge", rubric.edge, over_surface(DANGER, 0.4)),
            ("Indigo solid", indigo.solid, ACCENT),
            ("Indigo text", indigo.text, LINK),
            ("Indigo wash", indigo.wash, over_surface(ACCENT, 0.16)),
            ("Indigo edge", indigo.edge, over_surface(ACCENT, 0.38)),
            ("Purple wash", purple.wash, over_surface(purple.solid, 0.16)),
            ("Purple edge", purple.edge, over_surface(purple.solid, 0.38)),
            (
                "Verdigris wash",
                verdigris.wash,
                over_surface(verdigris.solid, 0.16),
            ),
            (
                "Verdigris edge",
                verdigris.edge,
                over_surface(verdigris.solid, 0.38),
            ),
            ("Ochre solid", ochre.solid, AMBER),
            ("Ochre text", ochre.text, AMBER),
            ("Ochre wash", ochre.wash, over_surface(AMBER, 0.16)),
            ("Ochre edge", ochre.edge, over_surface(AMBER, 0.38)),
            ("Madder wash", madder.wash, over_surface(madder.solid, 0.16)),
            ("Madder edge", madder.edge, over_surface(madder.solid, 0.38)),
            ("Success solid", success.solid, OK),
            ("Success text", success.text, OK),
            ("Success wash", success.wash, over_surface(OK, 0.16)),
            ("Success edge", success.edge, over_surface(OK, 0.38)),
            ("Warning solid", warning.solid, AMBER),
            ("Warning text", warning.text, AMBER),
            ("Warning wash", warning.wash, over_surface(AMBER, 0.16)),
            ("Warning edge", warning.edge, over_surface(AMBER, 0.38)),
            ("Danger solid", danger.solid, DANGER),
            ("Danger text", danger.text, DANGER),
            ("Danger wash", danger.wash, over_surface(DANGER, 0.16)),
            ("Danger edge", danger.edge, over_surface(DANGER, 0.4)),
        ];
        for (name, ours, expected) in rows {
            assert_eq!(ours, expected, "{name}");
        }
    }
}
