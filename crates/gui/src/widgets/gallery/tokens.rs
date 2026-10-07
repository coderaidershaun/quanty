//! The top of the gallery: the colour tokens, the tones, the text styles, every icon, and a line
//! of the symbols that the fonts must draw.

use eframe::egui::{self, Align2, Color32, Sense, Stroke, StrokeKind, vec2};

use super::{rows, section};
use crate::theme::{Icon, TextRole, Tone, color, radius, size, space, stroke};

const COLOURS: [(&str, Color32); 15] = [
    ("CANVAS", color::CANVAS),
    ("PANEL", color::PANEL),
    ("RAISED", color::RAISED),
    ("RAISED_HOVER", color::RAISED_HOVER),
    ("BORDER", color::BORDER),
    ("HAIRLINE", color::HAIRLINE),
    ("TEXT", color::TEXT),
    ("TEXT_SECONDARY", color::TEXT_SECONDARY),
    ("TEXT_MUTED", color::TEXT_MUTED),
    ("TEXT_ON_ACCENT", color::TEXT_ON_ACCENT),
    ("PAGE", color::PAGE),
    ("FOCUS", color::FOCUS),
    ("SELECTION", color::SELECTION),
    ("SCRIM", color::SCRIM),
    ("PAGE_HIGHLIGHT", color::PAGE_HIGHLIGHT),
];

const ROLES: [(&str, TextRole); 8] = [
    ("Title", TextRole::Title),
    ("Heading", TextRole::Heading),
    ("Body", TextRole::Body),
    ("BodyStrong", TextRole::BodyStrong),
    ("Label", TextRole::Label),
    ("Small", TextRole::Small),
    ("Micro", TextRole::Micro),
    ("Mono", TextRole::Mono),
];

const TILE: egui::Vec2 = vec2(112.0, size::CONTROL_LG);
/// A colour whose channels add up to more than this is light, so it gets dark text.
const LIGHT_SUM: u32 = 380;
/// The room for one icon and its name.
const ICON_CELL: f32 = 72.0;
const SYMBOLS: &str = "α β σ Δ Σ ∂ ∑ ∫ √ ∞ ≈ ≠ ≤ ≥ → ⇒ ↦ ∈ ∀ ℝ ℚ ℙ 𝔼 x² xᵢ ½ – — “q” ⌘K";

pub(super) fn show(ui: &mut egui::Ui) {
    section(ui, "Colours");
    rows(ui, TILE.x, &COLOURS, |ui, (name, colour)| {
        tile(ui, name, *colour)
    });
    section(ui, "Tones: solid, on_solid, text, wash, edge");
    for tone in Tone::ALL {
        let swatch = tone.swatch();
        ui.horizontal(|ui| {
            ui.set_width(ui.available_width());
            ui.label(TextRole::Label.rich(format!("{tone:?}")));
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
        ui.label(role.rich(sample));
    }
    section(ui, "Icons");
    rows(ui, ICON_CELL, Icon::ALL, |ui, (name, icon)| {
        ui.vertical(|ui| {
            ui.set_width(ICON_CELL);
            ui.label(icon.rich(size::ICON_LG));
            ui.label(TextRole::Micro.rich(*name).color(color::TEXT_MUTED));
        });
    });
    section(ui, "Symbols");
    ui.label(TextRole::Body.rich(format!("Symbols: {SYMBOLS}")));
}

/// A filled rectangle with the name of the colour and its hex value inside it.
fn tile(ui: &mut egui::Ui, name: &str, colour: Color32) {
    let (rect, _) = ui.allocate_exact_size(TILE, Sense::hover());
    let painter = ui.painter();
    // A colour with some transparency is shown on the panel colour, as it is used.
    painter.rect_filled(rect, radius::MD, color::PANEL);
    painter.rect(
        rect,
        radius::MD,
        colour,
        Stroke::new(stroke::BORDER, color::BORDER),
        StrokeKind::Inside,
    );
    let [r, g, b, _] = colour.to_array();
    let is_light = u32::from(r) + u32::from(g) + u32::from(b) > LIGHT_SUM;
    let ink = if is_light {
        color::TEXT_ON_ACCENT
    } else {
        color::TEXT
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

fn hex(colour: Color32) -> String {
    let [r, g, b, a] = colour.to_srgba_unmultiplied();
    if a == u8::MAX {
        format!("#{r:02X}{g:02X}{b:02X}")
    } else {
        format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
    }
}
