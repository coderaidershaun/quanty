//! The fonts of the app: one Inter file drawn at three weights, a math fallback, and the icon
//! font. Each weight is its own font family, so a `FontId` carries the weight it is drawn in.

use std::sync::{Arc, LazyLock};

use eframe::egui::epaint::text::VariationCoords;
use eframe::egui::{FontData, FontDefinitions, FontFamily, FontTweak};

const INTER: &[u8] = include_bytes!("../../assets/fonts/Inter-Variable.ttf");
const NOTO_MATH: &[u8] = include_bytes!("../../assets/fonts/NotoSansMath-Regular.ttf");

pub(super) static MEDIUM: LazyLock<FontFamily> =
    LazyLock::new(|| FontFamily::Name("medium".into()));
pub(super) static SEMIBOLD: LazyLock<FontFamily> =
    LazyLock::new(|| FontFamily::Name("semibold".into()));
pub(super) static ICONS: LazyLock<FontFamily> = LazyLock::new(|| FontFamily::Name("icons".into()));

/// What is tried, in order, when the first font of a family lacks a character.
const FALLBACKS: [&str; 4] = ["noto-math", "Hack", "NotoEmoji-Regular", "emoji-icon-font"];

fn inter(weight: f32) -> Arc<FontData> {
    Arc::new(FontData::from_static(INTER).tweak(FontTweak {
        coords: VariationCoords::new([("wght", weight)]),
        ..FontTweak::default()
    }))
}

fn text_family(first: &str) -> Vec<String> {
    std::iter::once(first)
        .chain(FALLBACKS)
        .map(str::to_owned)
        .collect()
}

/// Every font and family the app draws with. Hack and the emoji fonts come from eframe.
pub(super) fn definitions() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.remove("Ubuntu-Light");
    fonts.font_data.insert("inter-400".into(), inter(400.0));
    fonts.font_data.insert("inter-500".into(), inter(500.0));
    fonts.font_data.insert("inter-600".into(), inter(600.0));
    fonts.font_data.insert(
        "noto-math".into(),
        Arc::new(FontData::from_static(NOTO_MATH)),
    );
    fonts.font_data.insert(
        "phosphor".into(),
        Arc::new(egui_phosphor::Variant::Regular.font_data()),
    );
    fonts
        .families
        .insert(FontFamily::Proportional, text_family("inter-400"));
    fonts
        .families
        .insert(MEDIUM.clone(), text_family("inter-500"));
    fonts
        .families
        .insert(SEMIBOLD.clone(), text_family("inter-600"));
    fonts.families.insert(
        FontFamily::Monospace,
        vec![
            "Hack".to_owned(),
            "inter-400".to_owned(),
            "noto-math".to_owned(),
            "NotoEmoji-Regular".to_owned(),
            "emoji-icon-font".to_owned(),
        ],
    );
    fonts
        .families
        .insert(ICONS.clone(), vec!["phosphor".to_owned()]);
    fonts
}
