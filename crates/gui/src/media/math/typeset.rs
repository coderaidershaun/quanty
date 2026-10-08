//! Turns LaTeX into white ink on a clear ground, one formula at a time. It is the only file that
//! talks to the typesetting library, so a different library changes this file alone.

use image::{ImageFormat, RgbaImage};
use ratex_layout::{LayoutOptions, layout, to_display_list};
use ratex_parser::{ParseError, parser::parse};
use ratex_render::{RenderOptions, render_to_png};
use ratex_types::color::Color;
use ratex_types::display_item::DisplayList;
use ratex_types::math_style::MathStyle;

use super::image::Display;

const MAX_LATEX_BYTES: usize = 4096;
const MAX_PICTURE_SIDE: u32 = 8192;
const SOFT_HYPHEN: char = '\u{ad}';
const BLEED_EM: f64 = 0.1;
const MIN_BLEED_PX: u32 = 2;

/// White ink on a clear ground, with a clear margin of `bleed_px` pixels on every side. The
/// lengths in em are those of the formula's box, without the margin.
pub(super) struct Raster {
    pub(super) rgba: Vec<u8>,
    pub(super) size: [usize; 2],
    pub(super) width_em: f64,
    pub(super) ascent_em: f64,
    pub(super) descent_em: f64,
    pub(super) bleed_px: u32,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum TypesetError {
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
    #[error("the picture cannot be decoded")]
    Decode {
        #[from]
        source: image::ImageError,
    },
    #[error("the typesetting library stopped with a panic")]
    Panicked,
}

/// Typesets `latex` at `em_px` pixels to the em. `max_side` is the largest side a texture may
/// have: a bigger picture is refused before one pixel of it is made.
///
/// # Errors
/// Every way a formula can fail is a [`TypesetError`]; none of them is a reason to stop.
pub(super) fn typeset(
    latex: &str,
    display: Display,
    em_px: u16,
    max_side: u32,
) -> Result<Raster, TypesetError> {
    let list = lay_out(latex, display)?;
    check_box(&list)?;

    let bleed_px = bleed_for(em_px);
    let em = f64::from(em_px);
    let margin = 2 * u64::from(bleed_px);
    let pixels = |length_em: f64| ((length_em * em).ceil() as u64).saturating_add(margin);
    check_sides(
        pixels(list.width),
        pixels(list.height + list.depth),
        max_side,
    )?;

    let picture = draw(&list, em_px, bleed_px)?;
    // The library rounds in single precision, so the real picture can be a pixel larger than
    // the estimate above.
    check_sides(
        u64::from(picture.width()),
        u64::from(picture.height()),
        max_side,
    )?;
    Ok(Raster {
        size: [picture.width() as usize, picture.height() as usize],
        rgba: picture.into_raw(),
        width_em: list.width,
        ascent_em: list.height,
        descent_em: list.depth,
        bleed_px,
    })
}

fn lay_out(latex: &str, display: Display) -> Result<DisplayList, TypesetError> {
    let latex = clean(latex)?;
    let nodes = parse(&latex)?;
    let style = match display {
        Display::Block => MathStyle::Display,
        Display::Inline => MathStyle::Text,
    };
    let options = LayoutOptions::default()
        .with_style(style)
        .with_color(Color::WHITE);
    Ok(to_display_list(&layout(&nodes, &options)))
}

/// The typesetting library hands back the bytes of a PNG file, so the picture is decoded from
/// them here.
fn draw(list: &DisplayList, em_px: u16, bleed_px: u32) -> Result<RgbaImage, TypesetError> {
    let options = RenderOptions {
        font_size: f32::from(em_px),
        padding: bleed_px as f32,
        background_color: Color::new(0.0, 0.0, 0.0, 0.0),
        font_dir: String::new(),
        device_pixel_ratio: 1.0,
    };
    let png = render_to_png(list, &options).map_err(|message| TypesetError::Raster { message })?;
    Ok(image::load_from_memory_with_format(&png, ImageFormat::Png)?.into_rgba8())
}

/// Trims the source and drops soft hyphens, which the typesetting library would answer with a
/// slow scan of the system fonts.
fn clean(latex: &str) -> Result<String, TypesetError> {
    let without_hyphens: String = latex.chars().filter(|&c| c != SOFT_HYPHEN).collect();
    let latex = without_hyphens.trim();
    if latex.is_empty() {
        return Err(TypesetError::Empty);
    }
    if latex.len() > MAX_LATEX_BYTES {
        return Err(TypesetError::TooLong { bytes: latex.len() });
    }
    Ok(latex.to_owned())
}

/// Ink reaches a little outside the box of a formula, so the picture has a clear margin.
fn bleed_for(em_px: u16) -> u32 {
    ((BLEED_EM * f64::from(em_px)).ceil() as u32).max(MIN_BLEED_PX)
}

/// Negative space alone has a box narrower than nothing, and no caller can lay that out.
fn check_box(list: &DisplayList) -> Result<(), TypesetError> {
    let is_length = |length: f64| length.is_finite() && length >= 0.0;
    if [list.width, list.height, list.depth]
        .into_iter()
        .all(is_length)
    {
        return Ok(());
    }
    Err(TypesetError::BoxSize {
        width_em: list.width,
        ascent_em: list.height,
        descent_em: list.depth,
    })
}

fn check_sides(width_px: u64, height_px: u64, max_side: u32) -> Result<(), TypesetError> {
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
