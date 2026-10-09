//! The sizes a picture is kept at: its own pixels, then a half, a quarter, and so on. The graphics
//! card does not average pixels when it shrinks a texture, so a picture drawn small is kept small.

use image::RgbaImage;

pub(super) const MAX_LEVELS: usize = 6;

const SHORTEST_SIDE: u32 = 32;

/// Each pixel is the mean of a 2 by 2 block. An odd last row or column is dropped.
pub(super) fn halve(source: &RgbaImage) -> RgbaImage {
    let (width, height) = (source.width(), source.height());
    let mut half = RgbaImage::new((width / 2).max(1), (height / 2).max(1));
    for (x, y, pixel) in half.enumerate_pixels_mut() {
        let (left, top) = (2 * x, 2 * y);
        // A side of one pixel has no second pixel, so its block counts that pixel twice.
        let right = (left + 1).min(width - 1);
        let bottom = (top + 1).min(height - 1);
        let block = [(left, top), (right, top), (left, bottom), (right, bottom)]
            .map(|(block_x, block_y)| source.get_pixel(block_x, block_y).0);
        pixel.0 = mean(&block);
    }
    half
}

/// Each colour counts as much as its pixel is solid, so that the colour of a clear pixel does not
/// give a shape a dark edge. A block with no solid pixel is clear, and its colour is never seen.
fn mean(block: &[[u8; 4]; 4]) -> [u8; 4] {
    let mut colour = [0u32; 3];
    let mut alpha = 0u32;
    for &[red, green, blue, solid] in block {
        let weight = u32::from(solid);
        for (total, channel) in colour.iter_mut().zip([red, green, blue]) {
            *total += u32::from(channel) * weight;
        }
        alpha += weight;
    }
    let [red, green, blue] = if alpha == 0 {
        [0; 3]
    } else {
        colour.map(|total| ((total + alpha / 2) / alpha) as u8)
    };
    [red, green, blue, ((alpha + 2) / 4) as u8]
}

pub(super) fn levels(picture: RgbaImage, max_side: u32) -> Vec<RgbaImage> {
    let mut first = picture;
    // Halving stops at one pixel, so a limit below one pixel would never be met.
    while first.width().max(first.height()) > max_side.max(1) {
        first = halve(&first);
    }
    let mut chain = vec![first];
    while chain.len() < MAX_LEVELS {
        let last = &chain[chain.len() - 1];
        if (last.width() / 2).min(last.height() / 2) < SHORTEST_SIDE {
            break;
        }
        let next = halve(last);
        chain.push(next);
    }
    chain
}

#[cfg(test)]
mod tests {
    use image::{Rgba, RgbaImage};

    use super::halve;

    #[test]
    fn a_clear_pixel_does_not_darken_the_mean() {
        let mut block = RgbaImage::from_pixel(2, 2, Rgba([0, 0, 0, 0]));
        block.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        assert_eq!(halve(&block).get_pixel(0, 0).0, [255, 0, 0, 64]);
    }
}
