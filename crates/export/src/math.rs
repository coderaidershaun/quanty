//! Typesets a formula as a picture of black ink on a clear ground, for a page that is printed or
//! read in a browser. It is the only file that talks to the typesetting library, and it keeps the
//! formulas it has made, so a formula that comes twice is typeset once.

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use ratex_layout::{LayoutOptions, layout, to_display_list};
use ratex_parser::parser::parse;
use ratex_render::{RenderOptions, render_to_png};
use ratex_types::color::Color;
use ratex_types::display_item::DisplayList;
use ratex_types::math_style::MathStyle;

/// Pixels to the em of a picture: about 600 dots to the inch at the size of the body text.
const EM_PX: u16 = 96;
/// The size a formula is tried at once more when the first picture would be too large.
const SMALLER_EM_PX: u16 = 48;
// SMELL: the desktop app typesets the formulas of its window in white ink with these same limits
// and layout options. They are repeated here so that the window's code stays as it is, and a
// change to one must be made in the other.
const MAX_LATEX_BYTES: usize = 4096;
const MAX_PICTURE_SIDE: u64 = 8192;
const SOFT_HYPHEN: char = '\u{ad}';
const BLEED_EM: f64 = 0.1;
const MIN_BLEED_PX: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Display {
    Inline,
    Block,
}

/// A typeset formula. The lengths in em are those of the box of the formula. The picture is that
/// box and a clear margin of `bleed_em` on every side, because ink reaches a little outside the
/// box.
pub(crate) struct Formula {
    pub(crate) png: Vec<u8>,
    pub(crate) width_em: f64,
    pub(crate) ascent_em: f64,
    pub(crate) descent_em: f64,
    pub(crate) bleed_em: f64,
}

impl Formula {
    /// The whole picture's width in em, margin included.
    pub(crate) fn picture_width_em(&self) -> f64 {
        self.width_em + 2.0 * self.bleed_em
    }

    pub(crate) fn picture_height_em(&self) -> f64 {
        self.ascent_em + self.descent_em + 2.0 * self.bleed_em
    }
}

/// The formulas of one file, each typeset once. `None` stands for a formula that cannot be
/// typeset: the file then shows its LaTeX.
#[derive(Default)]
pub(crate) struct Formulas {
    made: HashMap<(String, Display), Option<Rc<Formula>>>,
}

impl Formulas {
    pub(crate) fn get(&mut self, latex: &str, display: Display) -> Option<Rc<Formula>> {
        self.made
            .entry((latex.to_owned(), display))
            .or_insert_with(|| typeset_safely(latex, display).map(Rc::new))
            .clone()
    }
}

/// The typesetting library can panic on a formula it does not expect, and no formula is a reason
/// to stop writing a file.
fn typeset_safely(latex: &str, display: Display) -> Option<Formula> {
    catch_unwind(AssertUnwindSafe(|| {
        typeset(latex, display, EM_PX).or_else(|| typeset(latex, display, SMALLER_EM_PX))
    }))
    .ok()
    .flatten()
}

fn typeset(latex: &str, display: Display, em_px: u16) -> Option<Formula> {
    let list = lay_out(latex, display)?;
    let is_length = |length: f64| length.is_finite() && length >= 0.0;
    if ![list.width, list.height, list.depth]
        .into_iter()
        .all(is_length)
    {
        return None;
    }
    let bleed_px = ((BLEED_EM * f64::from(em_px)).ceil() as u32).max(MIN_BLEED_PX);
    let em = f64::from(em_px);
    let estimate = |length_em: f64| ((length_em * em).ceil() as u64) + 2 * u64::from(bleed_px);
    if estimate(list.width) > MAX_PICTURE_SIDE
        || estimate(list.height + list.depth) > MAX_PICTURE_SIDE
    {
        return None;
    }
    let options = RenderOptions {
        font_size: f32::from(em_px),
        padding: bleed_px as f32,
        background_color: Color::new(0.0, 0.0, 0.0, 0.0),
        font_dir: String::new(),
        device_pixel_ratio: 1.0,
    };
    let png = render_to_png(&list, &options).ok()?;
    Some(Formula {
        png,
        width_em: list.width,
        ascent_em: list.height,
        descent_em: list.depth,
        bleed_em: f64::from(bleed_px) / em,
    })
}

/// Soft hyphens are dropped, because the typesetting library would answer them with a slow scan
/// of the fonts of the system.
fn lay_out(latex: &str, display: Display) -> Option<DisplayList> {
    let without_hyphens: String = latex.chars().filter(|&c| c != SOFT_HYPHEN).collect();
    let latex = without_hyphens.trim();
    if latex.is_empty() || latex.len() > MAX_LATEX_BYTES {
        return None;
    }
    let nodes = parse(latex).ok()?;
    let style = match display {
        Display::Block => MathStyle::Display,
        Display::Inline => MathStyle::Text,
    };
    let options = LayoutOptions::default()
        .with_style(style)
        .with_color(Color::BLACK);
    Some(to_display_list(&layout(&nodes, &options)))
}
