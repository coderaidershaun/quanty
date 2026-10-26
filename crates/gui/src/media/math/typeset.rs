//! Turns LaTeX into white ink on a clear ground, one formula at a time. The typesetting itself is
//! the export crate's, which also sets the ink of the files it writes; this file decodes its
//! picture into pixels.

use export::{TypesetError as Refused, check_sides, typeset_png};
use image::ImageFormat;

use super::image::Display;

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
    #[error(transparent)]
    Refused(#[from] Refused),
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
    let is_block = matches!(display, Display::Block);
    let made = typeset_png(latex, is_block, em_px, true, max_side)?;
    let picture = image::load_from_memory_with_format(&made.png, ImageFormat::Png)?.into_rgba8();
    // The library rounds in single precision, so the real picture can be a pixel larger than
    // the estimate it was checked by.
    check_sides(
        u64::from(picture.width()),
        u64::from(picture.height()),
        max_side,
    )?;
    Ok(Raster {
        size: [picture.width() as usize, picture.height() as usize],
        rgba: picture.into_raw(),
        width_em: made.width_em,
        ascent_em: made.ascent_em,
        descent_em: made.descent_em,
        bleed_px: made.bleed_px,
    })
}
