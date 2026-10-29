//! The two palettes keep the app readable, and are the only place where a colour is chosen.
//!
//! Both rules are fitness tests. Each holds over every colour of both palettes or over all the
//! source, and each fails silently: text that is too faint is still drawn, and a colour made by
//! hand is still drawn, only wrong in one of the palettes.

use std::path::Path;

use eframe::egui::Color32;
use gui::theme::{Palette, Scheme, Tone};

use crate::boundaries::{manifest_folder, read, rust_files};

/// What WCAG 2 asks of text, and of an icon, a ring or the outline of a control.
const TEXT_RATIO: f64 = 4.5;
const MARK_RATIO: f64 = 3.0;

/// What makes a colour from numbers or from text, with the names after it that are not a choice
/// of colour: clear, and "the colour comes later". A match counts only where the name starts, so
/// `RgbaImage::` and `ColorImage::` (which decode a picture) do not match. Any path in front is
/// fine: `egui::`, `eframe::egui::`, `epaint::` or none.
const MAKERS: [(&str, &[&str]); 5] = [
    ("Color32::", &["TRANSPARENT", "PLACEHOLDER"]),
    ("Rgba::", &[]),
    ("Hsva::", &[]),
    ("HsvaGamma::", &[]),
    ("hex_color!", &[]),
];

fn makes_a_colour(line: &str) -> bool {
    MAKERS.iter().any(|(maker, allowed)| {
        line.match_indices(maker).any(|(at, _)| {
            let ends_a_longer_name = line[..at]
                .chars()
                .next_back()
                .is_some_and(|before| before.is_alphanumeric() || before == '_');
            let name: String = line[at + maker.len()..]
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            !ends_a_longer_name && !allowed.contains(&name.as_str())
        })
    })
}

/// The dark desk and the dark text, as a web page writes a colour.
fn hex(color: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b())
}

/// Every line under `folder`, except in `except`, that makes a colour, as `file:line: text`.
fn colours_made_by_hand(folder: &Path, except: &Path) -> Vec<String> {
    let mut found = Vec::new();
    for file in rust_files(folder) {
        if file.starts_with(except) {
            continue;
        }
        for (number, line) in read(&file).lines().enumerate() {
            if !line.trim_start().starts_with("//") && makes_a_colour(line) {
                found.push(format!(
                    "{}:{}: {}",
                    file.display(),
                    number + 1,
                    line.trim()
                ));
            }
        }
    }
    found
}

/// A colour made by hand outside the theme is the same in both palettes, so one of them draws it
/// wrong: white text on a dark desk, or a dark mark on a dark sheet. Nothing fails when it is
/// drawn, so only the source can say. Methods on a palette colour, such as `gamma_multiply`, make
/// no new colour and are allowed.
///
/// The web page holds two copies of the dark palette by hand, because it is drawn before the app
/// starts and shows in the margins of a phone's safe area: the desk (the page, the theme colour of
/// the browser and the manifest) and the text (the loading words). They must equal the palette.
#[test]
fn no_colour_is_made_outside_the_theme() {
    let source = manifest_folder().join("src");
    let breaks = colours_made_by_hand(&source, &source.join("theme"));
    assert!(
        breaks.is_empty(),
        "a colour is made outside the theme, so one palette draws it wrong:\n{}",
        breaks.join("\n")
    );

    let web = manifest_folder()
        .parent()
        .expect("the gui crate stands in the crates folder")
        .join("web-ui");
    let copies = [
        ("index.html", "desk", Palette::DARK.desk),
        ("index.html", "text", Palette::DARK.text),
        ("assets/manifest.json", "desk", Palette::DARK.desk),
    ];
    for (file, name, color) in copies {
        let path = web.join(file);
        assert!(
            read(&path).to_lowercase().contains(&hex(color)),
            "{} no longer holds {}, the dark {name}: the page shows another colour until the app draws",
            path.display(),
            hex(color)
        );
    }
}

/// One ink on one ground, with the ratio that WCAG 2 asks for it.
struct Pair {
    label: String,
    ink: Color32,
    ground: Color32,
    needs: f64,
}

fn luminance(color: Color32) -> f64 {
    assert_eq!(
        color.a(),
        u8::MAX,
        "{color:?} is see-through, so its contrast depends on what lies behind it"
    );
    let linear = |byte: u8| {
        let value = f64::from(byte) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
}

fn contrast(ink: Color32, ground: Color32) -> f64 {
    let (a, b) = (luminance(ink), luminance(ground));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// The texts that are held to 4.5:1 on `well_deep`, the fill of a pressed row, card, result card
/// or suggested question, and of a toggle that is chosen while the pointer is over it. Only
/// `text` and `text_secondary` are held there. `text_muted` and the tone texts are left out,
/// because that fill shows under a list row, a card, a result card or a suggested question only
/// while the pointer button is held.
fn texts_on_the_pressed_fill(palette: &Palette) -> Vec<(String, Color32)> {
    vec![
        ("text".to_owned(), palette.text),
        ("text_secondary".to_owned(), palette.text_secondary),
    ]
}

/// Every pair that is checked, for one palette.
fn pairs(palette: &Palette) -> Vec<Pair> {
    let mut found = Vec::new();
    let mut add = |ink: &str, ink_color: Color32, grounds: &[(&str, Color32)], needs: f64| {
        for &(ground, ground_color) in grounds {
            found.push(Pair {
                label: format!("{ink} on {ground}"),
                ink: ink_color,
                ground: ground_color,
                needs,
            });
        }
    };
    let desk = ("desk", palette.desk);
    let sheet = ("sheet", palette.sheet);
    let well = ("well", palette.well);
    let paper = [desk, sheet, well];

    for (name, color) in [
        ("text", palette.text),
        ("text_heading", palette.text_heading),
        ("text_subheading", palette.text_subheading),
        ("text_secondary", palette.text_secondary),
        ("text_muted", palette.text_muted),
    ] {
        add(name, color, &paper, TEXT_RATIO);
    }
    for (name, color) in texts_on_the_pressed_fill(palette) {
        add(
            &name,
            color,
            &[("well_deep", palette.well_deep)],
            TEXT_RATIO,
        );
    }
    add(
        "text",
        palette.text,
        &[("selection", palette.selection)],
        TEXT_RATIO,
    );
    add(
        "text_on_ink",
        palette.text_on_ink,
        &[
            ("ink", palette.ink),
            ("ink_hover", palette.ink_hover),
            ("ink_pressed", palette.ink_pressed),
        ],
        TEXT_RATIO,
    );
    for tone in Tone::ALL {
        let swatch = palette.tone(tone);
        let text = format!("{tone:?} text");
        add(&text, swatch.text, &paper, TEXT_RATIO);
        add(&text, swatch.text, &[("its wash", swatch.wash)], TEXT_RATIO);
        let solid = format!("{tone:?} solid");
        add(&solid, swatch.solid, &[desk, sheet], MARK_RATIO);
    }
    let rubric = palette.tone(Tone::Rubric);
    for (name, color) in [
        ("text", palette.text),
        ("text_secondary", palette.text_secondary),
        ("text_muted", palette.text_muted),
    ] {
        add(name, color, &[("Rubric wash", rubric.wash)], TEXT_RATIO);
    }
    for tone in [Tone::Rubric, Tone::Danger] {
        let swatch = palette.tone(tone);
        let on_solid = format!("{tone:?} on_solid");
        add(
            &on_solid,
            swatch.on_solid,
            &[("its solid", swatch.solid)],
            TEXT_RATIO,
        );
    }

    add("border", palette.border, &[desk, sheet], MARK_RATIO);
    add("focus", palette.focus, &paper, MARK_RATIO);
    add("rule", palette.rule, &paper, MARK_RATIO);
    add(
        "Rubric solid",
        rubric.solid,
        &[("page", palette.page)],
        MARK_RATIO,
    );
    add(
        "ink",
        palette.ink,
        &[("hairline", palette.hairline)],
        MARK_RATIO,
    );
    found
}

/// Every text and mark meets WCAG 2 AA in both palettes, on each surface it is drawn on. The
/// ratio is the one of relative luminance, from the sRGB bytes.
///
/// Text, 4.5:1:
/// - `text`, `text_heading`, `text_secondary` and `text_muted` on `desk`, `sheet` and `well`.
/// - `text` and `text_secondary` on `well_deep`, the fill of a pressed row, card, result card or
///   suggested question, and of a toggle that is chosen while the pointer is over it.
/// - `text` on `selection`, for selected text.
/// - `text_on_ink` on `ink`, `ink_hover` and `ink_pressed`, for the main button.
/// - The `text` of each tone on `desk`, `sheet`, `well` and its own `wash`, for chips, the legend,
///   graph nodes, notices and the danger button.
/// - `text`, `text_secondary` and `text_muted` on the `wash` of Rubric, for the marked piece in
///   the page-text view.
/// - `on_solid` of Rubric and of Danger on their `solid`, for the chosen citation chip.
///
/// Large text, 3:1: only `Title` (24 pt) is large text, and it uses `text_heading`, which is
/// already held to 4.5:1. The large-text rule adds no pair.
///
/// Marks, 3:1:
/// - `border` on `desk` and `sheet`, for the outline of a control against what is around it.
/// - `focus`, and `rule`, on `desk`, `sheet` and `well`, for the keyboard ring, a chosen
///   outline, the mark of the open tab and a heavy rule.
/// - The `solid` of each tone on `desk` and `sheet`, for graph-node outlines, legend dots and the
///   outline of the rubric.
/// - The `solid` of Rubric on `page`, for the outline of the cited figure on the white scan.
/// - `ink` on `hairline`, for the filled part of a progress bar on its track.
///
/// Not checked, and why:
/// - `text_faint`: it is only the big icon of an empty state and the number of a step not yet
///   reached, and both have words beside them.
/// - `hairline`: it is a decorative rule.
/// - The `edge` of each tone: it is the outline of a notice, whose words and icon carry the
///   meaning.
/// - `on_solid` of the other tones: no text is drawn on those solids.
/// - `scrim`, `shadow` and `page_highlight`: they are neither text nor the boundary of a control.
/// - Disabled controls, which are drawn faded: WCAG leaves inactive components out.
/// - `text_muted` and the tone texts on `well_deep`: see `texts_on_the_pressed_fill`.
#[test]
fn every_text_meets_aa_on_the_surfaces_it_is_drawn_on() {
    let mut failures = Vec::new();
    for scheme in Scheme::ALL {
        for pair in pairs(scheme.palette()) {
            let ratio = contrast(pair.ink, pair.ground);
            if ratio < pair.needs {
                failures.push(format!(
                    "{}: {} is {ratio:.2} and needs {:.1}",
                    scheme.as_stored(),
                    pair.label,
                    pair.needs
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "text or a mark is too faint to read on the surface it is drawn on:\n{}",
        failures.join("\n")
    );
}
