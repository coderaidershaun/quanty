//! Typesets a formula as a picture of ink on a clear ground. It is the only file that talks to the
//! typesetting library. The browser app takes `typeset_png` for white ink; the files this crate
//! writes take black ink, and keep the formulas they have made, so a formula that comes twice is
//! typeset once.

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use ratex_layout::{LayoutOptions, layout, to_display_list};
use ratex_parser::{ParseError, parser::parse};
use ratex_render::{RenderOptions, render_to_png};
use ratex_types::color::Color;
use ratex_types::display_item::DisplayList;
use ratex_types::math_style::MathStyle;

/// Pixels to the em of a picture: about 600 dots to the inch at the size of the body text.
const EM_PX: u16 = 96;
/// The size a formula is tried at once more when the first picture would be too large.
const SMALLER_EM_PX: u16 = 48;
const MAX_LATEX_BYTES: usize = 4096;
const MAX_PICTURE_SIDE: u32 = 8192;
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
    let is_block = display == Display::Block;
    let made = typeset_png(latex, is_block, em_px, false, MAX_PICTURE_SIDE).ok()?;
    Some(Formula {
        png: made.png,
        width_em: made.width_em,
        ascent_em: made.ascent_em,
        descent_em: made.descent_em,
        bleed_em: f64::from(made.bleed_px) / f64::from(em_px),
    })
}

/// A formula as a PNG: the lengths in em are those of its box, and the picture is that box and a
/// clear margin of `bleed_px` pixels on every side, because ink reaches a little outside the box.
pub struct TypesetPng {
    pub png: Vec<u8>,
    pub width_em: f64,
    pub ascent_em: f64,
    pub descent_em: f64,
    pub bleed_px: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum TypesetError {
    #[error("the formula is empty")]
    Empty,
    #[error("the formula is {bytes} bytes long, and the limit is {limit}", limit = MAX_LATEX_BYTES)]
    TooLong { bytes: usize },
    #[error("the formula cannot be read")]
    Parse {
        #[from]
        source: ParseError,
    },
    #[error(
        "the box of the formula is not a size: {width_em} em wide, {ascent_em} em above the baseline and {descent_em} em below it"
    )]
    BoxSize {
        width_em: f64,
        ascent_em: f64,
        descent_em: f64,
    },
    #[error("the picture would be {width_px} by {height_px} pixels, and the limit is {limit}")]
    TooLarge {
        width_px: u64,
        height_px: u64,
        limit: u32,
    },
    #[error("the picture cannot be drawn: {message}")]
    Raster { message: String },
}

/// Typesets `latex` at `em_px` pixels to the em, as black ink or as white ink. `max_side` is the
/// largest side a picture may have: a bigger one is refused before one pixel of it is made.
///
/// # Errors
/// Every way a formula can fail is a [`TypesetError`]; none of them is a reason to stop.
pub fn typeset_png(
    latex: &str,
    is_block: bool,
    em_px: u16,
    is_white: bool,
    max_side: u32,
) -> Result<TypesetPng, TypesetError> {
    let list = lay_out(latex, is_block, is_white)?;
    // Negative space alone has a box narrower than nothing, and no caller can lay that out.
    let is_length = |length: f64| length.is_finite() && length >= 0.0;
    if ![list.width, list.height, list.depth]
        .into_iter()
        .all(is_length)
    {
        return Err(TypesetError::BoxSize {
            width_em: list.width,
            ascent_em: list.height,
            descent_em: list.depth,
        });
    }
    let bleed_px = ((BLEED_EM * f64::from(em_px)).ceil() as u32).max(MIN_BLEED_PX);
    let margin = 2 * u64::from(bleed_px);
    let pixels =
        |length_em: f64| ((length_em * f64::from(em_px)).ceil() as u64).saturating_add(margin);
    check_sides(
        pixels(list.width),
        pixels(list.height + list.depth),
        max_side,
    )?;
    let options = RenderOptions {
        font_size: f32::from(em_px),
        padding: bleed_px as f32,
        background_color: Color::new(0.0, 0.0, 0.0, 0.0),
        font_dir: String::new(),
        device_pixel_ratio: 1.0,
    };
    let png = render_to_png(&list, &options).map_err(|message| TypesetError::Raster { message })?;
    Ok(TypesetPng {
        png,
        width_em: list.width,
        ascent_em: list.height,
        descent_em: list.depth,
        bleed_px,
    })
}

/// Refuses a picture with a side over `max_side`, or over the limit of every picture.
///
/// # Errors
/// [`TypesetError::TooLarge`] when a side is over the limit.
pub fn check_sides(width_px: u64, height_px: u64, max_side: u32) -> Result<(), TypesetError> {
    let limit = max_side.min(MAX_PICTURE_SIDE);
    if width_px > u64::from(limit) || height_px > u64::from(limit) {
        return Err(TypesetError::TooLarge {
            width_px,
            height_px,
            limit,
        });
    }
    Ok(())
}

/// Soft hyphens are dropped, because the typesetting library would answer them with a slow scan
/// of the fonts of the system.
fn lay_out(latex: &str, is_block: bool, is_white: bool) -> Result<DisplayList, TypesetError> {
    let without_hyphens: String = latex.chars().filter(|&c| c != SOFT_HYPHEN).collect();
    let latex = without_hyphens.trim();
    if latex.is_empty() {
        return Err(TypesetError::Empty);
    }
    if latex.len() > MAX_LATEX_BYTES {
        return Err(TypesetError::TooLong { bytes: latex.len() });
    }
    let nodes = parse(latex)?;
    let style = if is_block {
        MathStyle::Display
    } else {
        MathStyle::Text
    };
    let ink = if is_white { Color::WHITE } else { Color::BLACK };
    let options = LayoutOptions::default().with_style(style).with_color(ink);
    Ok(to_display_list(&layout(&nodes, &options)))
}
