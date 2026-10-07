//! The sizes a picture is kept at: its own pixels, then a half, a quarter, and so on. The
//! graphics card does not average pixels when it shrinks a texture, so a picture drawn small
//! has to be stored small.

use image::RgbaImage;

pub(super) const MAX_LEVELS: usize = 6;

const SHORTEST_SIDE: u32 = 32;

/// Each pixel is the mean of a 2 by 2 block. An odd last row or column is dropped.
// SMELL: the colour of a clear pixel counts in the mean as much as that of a solid one, so a
// picture with clear parts can get a dark edge around its shapes in the smaller sizes.
pub(super) fn halve(source: &RgbaImage) -> RgbaImage {
    let (width, height) = (source.width(), source.height());
    let mut half = RgbaImage::new((width / 2).max(1), (height / 2).max(1));
    for (x, y, pixel) in half.enumerate_pixels_mut() {
        let (left, top) = (2 * x, 2 * y);
        // A side of one pixel has no second pixel, so its block counts that pixel twice.
        let right = (left + 1).min(width - 1);
        let bottom = (top + 1).min(height - 1);
        let mut sum = [0u32; 4];
        for (block_x, block_y) in [(left, top), (right, top), (left, bottom), (right, bottom)] {
            let channels = source.get_pixel(block_x, block_y).0;
            for (total, channel) in sum.iter_mut().zip(channels) {
                *total += u32::from(channel);
            }
        }
        pixel.0 = sum.map(|total| ((total + 2) / 4) as u8);
    }
    half
}

/// The picture and its halves, largest first. A picture whose longer side is above `max_side`
/// is halved until it fits, and that smaller copy is the first level.
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
