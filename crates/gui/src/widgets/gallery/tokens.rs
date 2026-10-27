//! The left column of the gallery: the colour tokens, the tones, the text styles, every icon,
//! and a line of the symbols that the fonts must draw.

use eframe::egui::{self, Align2, Color32, Sense, Stroke, StrokeKind, vec2};

use super::{rows, section};
use crate::theme::{self, Icon, Palette, TextRole, Tone, radius, size, space, stroke};

fn colours(palette: &Palette) -> [(&'static str, Color32); 22] {
    [
        ("DESK", palette.desk),
        ("SHEET", palette.sheet),
        ("WELL", palette.well),
        ("WELL_DEEP", palette.well_deep),
        ("HAIRLINE", palette.hairline),
        ("BORDER", palette.border),
        ("RULE", palette.rule),
        ("TEXT", palette.text),
        ("TEXT_HEADING", palette.text_heading),
        ("TEXT_SECONDARY", palette.text_secondary),
        ("TEXT_MUTED", palette.text_muted),
        ("TEXT_FAINT", palette.text_faint),
        ("TEXT_ON_INK", palette.text_on_ink),
        ("INK", palette.ink),
        ("INK_HOVER", palette.ink_hover),
        ("INK_PRESSED", palette.ink_pressed),
        ("PAGE", palette.page),
        ("FOCUS", palette.focus),
        ("SELECTION", palette.selection),
        ("SCRIM", palette.scrim),
        ("PAGE_HIGHLIGHT", palette.page_highlight),
        ("SHADOW", palette.shadow),
    ]
}

const ROLES: [(&str, TextRole); 10] = [
    ("Title", TextRole::Title),
    ("Heading", TextRole::Heading),
    ("Body", TextRole::Body),
    ("BodyStrong", TextRole::BodyStrong),
    ("Caption", TextRole::Caption),
    ("Label", TextRole::Label),
    ("Small", TextRole::Small),
    ("Micro", TextRole::Micro),
    ("Caps", TextRole::Caps),
    ("Mono", TextRole::Mono),
];

const TILE: egui::Vec2 = vec2(112.0, size::CONTROL_LG);
/// A colour whose channels add up to more than this is light, so its label takes the darker of the
/// two inks.
const LIGHT_SUM: u32 = 380;
const ICON_CELL: f32 = 72.0;
const SYMBOLS: &str = "α β σ Δ Σ ∂ ∑ ∫ √ ∞ ≈ ≠ ≤ ≥ → ⇒ ↦ ∈ ∀ ℝ ℚ ℙ 𝔼 ∇ 𝑎 x² x₁ ½ – — “q” ⌘K";

pub(super) fn show(ui: &mut egui::Ui) {
    let palette = theme::palette(ui.ctx());
    section(ui, "Colours");
    rows(ui, TILE.x, &colours(palette), |ui, (name, colour)| {
        tile(ui, name, *colour)
    });
    section(ui, "Tones: solid, on_solid, text, wash, edge");
    for tone in Tone::ALL {
        let swatch = palette.tone(tone);
        ui.horizontal(|ui| {
            ui.set_width(ui.available_width());
            ui.label(TextRole::Label.rich(ui, format!("{tone:?}")));
            for (name, colour) in [
                ("solid", swatch.solid),
                ("on_solid", swatch.on_solid),
                ("text", swatch.text),
                ("wash", swatch.wash),
                ("edge", swatch.edge),
            ] {
                tile(ui, name, colour);
            }
        });
    }
    section(ui, "Text styles");
    for (name, role) in ROLES {
        let font = role.font();
        let sample = format!(
            "{name}  {} / {}  Black–Scholes",
            font.size,
            role.line_height()
        );
        // The widgets that use this role paint its text in capitals, so the sample does too.
        let sample = if role == TextRole::Caps {
            sample.to_uppercase()
        } else {
            sample
        };
        ui.label(role.rich(ui, sample));
    }
    section(ui, "Icons");
    rows(ui, ICON_CELL, Icon::ALL, |ui, (name, icon)| {
        ui.vertical(|ui| {
            ui.set_width(ICON_CELL);
            ui.label(icon.rich(ui, size::ICON_LG));
            ui.label(TextRole::Micro.rich(ui, *name).color(palette.text_muted));
        });
    });
    section(ui, "Symbols");
    ui.label(TextRole::Body.rich(ui, format!("Symbols: {SYMBOLS}")));
}

fn tile(ui: &mut egui::Ui, name: &str, colour: Color32) {
    let palette = theme::palette(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(TILE, Sense::hover());
    let painter = ui.painter();
    // A colour with some transparency is shown on the sheet, as it is used.
    painter.rect_filled(rect, radius::MD, palette.sheet);
    painter.rect(
        rect,
        radius::MD,
        colour,
        Stroke::new(stroke::BORDER, palette.border),
        StrokeKind::Inside,
    );
    // The text colour is the dark ink in one palette and the light ink in the other.
    let (ink_on_light, ink_on_dark) = darker_and_lighter(palette.text, palette.text_on_ink);
    let ink = if brightness(palette.sheet.blend(colour)) > LIGHT_SUM {
        ink_on_light
    } else {
        ink_on_dark
    };
    let at = rect.left_top() + vec2(space::SM, space::XS);
    painter.text(at, Align2::LEFT_TOP, name, TextRole::Micro.font(), ink);
    let at = at + vec2(0.0, TextRole::Micro.line_height());
    painter.text(
        at,
        Align2::LEFT_TOP,
        hex(colour),
        TextRole::Micro.font(),
        ink,
    );
}

/// The sum of the channels, which is all that is needed to tell a light colour from a dark one.
fn brightness(colour: Color32) -> u32 {
    let [r, g, b, _] = colour.to_array();
    u32::from(r) + u32::from(g) + u32::from(b)
}

/// The two colours, the darker one first.
fn darker_and_lighter(a: Color32, b: Color32) -> (Color32, Color32) {
    if brightness(a) <= brightness(b) {
        (a, b)
    } else {
        (b, a)
    }
}

fn hex(colour: Color32) -> String {
    let [r, g, b, a] = colour.to_srgba_unmultiplied();
    if a == u8::MAX {
        format!("#{r:02X}{g:02X}{b:02X}")
    } else {
        format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
    }
}
