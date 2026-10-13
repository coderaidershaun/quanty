//! The fonts of the app: a book serif for what a person reads, a plain sans for controls, a code
//! font, a math fallback and the icon font. Each weight and style is its own font family, so a
//! `FontId` carries the weight and the style it is drawn in.

use std::sync::{Arc, LazyLock};

use eframe::egui::epaint::text::VariationCoords;
use eframe::egui::{FontData, FontDefinitions, FontFamily, FontTweak};

const STIX_MATH: &[u8] = include_bytes!("../../assets/fonts/STIXTwoMath-Regular.otf");

/// A font file for text, and how far down the math fallback is drawn under it, as a share of the
/// font size. egui centres a fallback font on the line of the font it stands in for, and STIX Two
/// Math declares a shorter line than these files, so with no shift its signs float above the
/// baseline. The shift is the gap between the two baselines that the ascent, descent and line gap
/// of each file give.
#[derive(Clone, Copy)]
struct TextFile {
    bytes: &'static [u8],
    math_shift: f32,
}

const LITERATA: TextFile = TextFile {
    bytes: include_bytes!("../../assets/fonts/Literata[opsz,wght].ttf"),
    math_shift: 0.2975,
};
const LITERATA_ITALIC: TextFile = TextFile {
    bytes: include_bytes!("../../assets/fonts/Literata-Italic[opsz,wght].ttf"),
    math_shift: 0.2975,
};
const SOURCE_SANS: TextFile = TextFile {
    bytes: include_bytes!("../../assets/fonts/SourceSans3[wght].ttf"),
    math_shift: 0.175,
};
const SOURCE_CODE: TextFile = TextFile {
    bytes: include_bytes!("../../assets/fonts/SourceCodePro[wght].ttf"),
    math_shift: 0.2185,
};

macro_rules! named_families {
    ($($family:ident = $name:literal),+ $(,)?) => {
        $(pub(super) static $family: LazyLock<FontFamily> =
            LazyLock::new(|| FontFamily::Name($name.into()));)+
    };
}

named_families! {
    SERIF = "serif",
    SERIF_SEMIBOLD = "serif-semibold",
    SERIF_DISPLAY = "serif-display",
    SERIF_ITALIC = "serif-italic",
    SERIF_SEMIBOLD_ITALIC = "serif-semibold-italic",
    SANS_MEDIUM = "sans-medium",
    SANS_SEMIBOLD = "sans-semibold",
    ICONS = "icons",
}

const FALLBACKS: [&str; 3] = ["Hack", "NotoEmoji-Regular", "emoji-icon-font"];

/// One font file drawn at one setting of its axes. A file with no optical size axis has `None`.
struct Face {
    file: TextFile,
    weight: f32,
    optical_size: Option<f32>,
}

impl Face {
    const fn new(file: TextFile, weight: f32, optical_size: Option<f32>) -> Self {
        Face {
            file,
            weight,
            optical_size,
        }
    }

    fn data(&self) -> Arc<FontData> {
        let mut coords = VariationCoords::new([("wght", self.weight)]);
        if let Some(optical_size) = self.optical_size {
            coords.push("opsz", optical_size);
        }
        Arc::new(FontData::from_static(self.file.bytes).tweak(FontTweak {
            coords,
            ..FontTweak::default()
        }))
    }

    /// STIX Two Math, moved down so that its signs stand on the baseline of this face.
    fn math_data(&self) -> Arc<FontData> {
        Arc::new(FontData::from_static(STIX_MATH).tweak(FontTweak {
            y_offset_factor: self.file.math_shift,
            ..FontTweak::default()
        }))
    }
}

/// The text of a family: its own font, its copy of the math fallback, then the fallbacks for the
/// signs that both lack.
fn text_family(first: &str, math: &str) -> Vec<String> {
    [first, math]
        .into_iter()
        .chain(FALLBACKS)
        .map(str::to_owned)
        .collect()
}

/// Hack and the emoji fonts come from eframe.
pub(super) fn definitions() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    // No family below names the text font that egui starts with, so it is not kept.
    fonts.font_data.remove("Ubuntu-Light");
    fonts.font_data.insert(
        "phosphor".into(),
        Arc::new(egui_phosphor::Variant::Light.font_data()),
    );
    let faces = [
        (SERIF.clone(), Face::new(LITERATA, 400.0, Some(14.0))),
        (
            SERIF_SEMIBOLD.clone(),
            Face::new(LITERATA, 600.0, Some(14.0)),
        ),
        (
            SERIF_DISPLAY.clone(),
            Face::new(LITERATA, 500.0, Some(36.0)),
        ),
        (
            SERIF_ITALIC.clone(),
            Face::new(LITERATA_ITALIC, 400.0, Some(14.0)),
        ),
        (
            SERIF_SEMIBOLD_ITALIC.clone(),
            Face::new(LITERATA_ITALIC, 600.0, Some(14.0)),
        ),
        (
            FontFamily::Proportional,
            Face::new(SOURCE_SANS, 400.0, None),
        ),
        (SANS_MEDIUM.clone(), Face::new(SOURCE_SANS, 500.0, None)),
        (SANS_SEMIBOLD.clone(), Face::new(SOURCE_SANS, 600.0, None)),
        (FontFamily::Monospace, Face::new(SOURCE_CODE, 400.0, None)),
    ];
    for (family, face) in faces {
        let key = family.to_string();
        let math_key = format!("{key} math");
        fonts.font_data.insert(key.clone(), face.data());
        fonts.font_data.insert(math_key.clone(), face.math_data());
        fonts.families.insert(family, text_family(&key, &math_key));
    }
    fonts
        .families
        .insert(ICONS.clone(), vec!["phosphor".to_owned()]);
    fonts
}
