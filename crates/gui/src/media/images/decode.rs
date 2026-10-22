//! Fetched bytes are decoded into bounded picture sizes and uploaded as textures.

use std::error::Error as _;
use std::io::Cursor;
use std::panic::{AssertUnwindSafe, catch_unwind};

use eframe::egui;

use super::chain;
use super::picture::ImageFailure;

/// A file beyond these is refused, so a damaged header cannot ask for all the memory.
const MAX_FILE_SIDE: u32 = 20_000;
const MAX_DECODE_BYTES: u64 = 1 << 30;

pub(super) struct DecodeJob {
    pub(super) url: String,
    pub(super) bytes: Result<Vec<u8>, String>,
    /// The longest side a stored size may have.
    pub(super) max_side: u32,
}

pub(super) struct Decoded {
    pub(super) url: String,
    pub(super) outcome: Result<Loaded, ImageFailure>,
}

/// The stored sizes of one picture, widest first. Dropping this frees the textures.
pub(super) struct Loaded {
    /// The file's own width and height, whatever size was stored.
    pub(super) size: egui::Vec2,
    pub(super) textures: Vec<egui::TextureHandle>,
}

#[derive(Debug, thiserror::Error)]
enum DecodeError {
    #[error("cannot decode {url}")]
    Decode {
        url: String,
        #[source]
        source: image::ImageError,
    },
    #[error("the decoder panicked on {url}")]
    Panicked { url: String },
}

/// A failure is logged once and handed back as a state.
pub(super) fn decode(ctx: &egui::Context, job: DecodeJob) -> Decoded {
    let DecodeJob {
        url,
        bytes,
        max_side,
    } = job;
    let outcome = match bytes {
        Ok(bytes) => catch_unwind(AssertUnwindSafe(|| load(ctx, &url, &bytes, max_side)))
            .unwrap_or_else(|_| Err(DecodeError::Panicked { url: url.clone() }))
            .map_err(|error| {
                let cause = error.source().map(tracing::field::display);
                tracing::warn!(%error, cause, "a picture cannot be shown");
                ImageFailure::Unreadable
            }),
        Err(reason) => {
            tracing::warn!(%url, %reason, "a picture cannot be fetched");
            Err(ImageFailure::Missing)
        }
    };
    Decoded { url, outcome }
}

fn load(
    ctx: &egui::Context,
    url: &str,
    bytes: &[u8],
    max_side: u32,
) -> Result<Loaded, DecodeError> {
    let format = image::guess_format(bytes).map_err(|source| DecodeError::Decode {
        url: url.to_owned(),
        source,
    })?;
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_FILE_SIDE);
    limits.max_image_height = Some(MAX_FILE_SIDE);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    reader.limits(limits);
    let decoded = reader.decode().map_err(|source| DecodeError::Decode {
        url: url.to_owned(),
        source,
    })?;

    let picture = decoded.into_rgba8();
    let size = egui::vec2(picture.width() as f32, picture.height() as f32);
    let textures = chain::levels(picture, max_side)
        .into_iter()
        .enumerate()
        .map(|(level, stored)| {
            let pixels = [stored.width() as usize, stored.height() as usize];
            let colors = egui::ColorImage::from_rgba_unmultiplied(pixels, stored.as_raw());
            let name = format!("image:{url}#{level}");
            ctx.load_texture(name, colors, egui::TextureOptions::LINEAR)
        })
        .collect();
    Ok(Loaded { size, textures })
}
