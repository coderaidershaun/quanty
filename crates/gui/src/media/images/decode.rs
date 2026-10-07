//! The work of a picture thread: read a file, decode it, make its stored sizes and upload them.
//! It is the only code that opens or decodes a picture file.

use std::error::Error as _;
use std::fs::File;
use std::io::{self, BufReader};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use eframe::egui;

use super::chain;
use super::picture::ImageFailure;

/// A file beyond these is refused, so a damaged header cannot ask for all the memory.
const MAX_FILE_SIDE: u32 = 20_000;
const MAX_DECODE_BYTES: u64 = 1 << 30;

pub(super) struct DecodeJob {
    pub(super) path: PathBuf,
    /// The longest side a stored size may have.
    pub(super) max_side: u32,
}

pub(super) struct Decoded {
    pub(super) path: PathBuf,
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
    #[error("no file at {}", path.display())]
    Missing { path: PathBuf },
    #[error("cannot read {}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot decode {}", path.display())]
    Decode {
        path: PathBuf,
        #[source]
        source: image::ImageError,
    },
    #[error("the decoder panicked on {}", path.display())]
    Panicked { path: PathBuf },
}

impl DecodeError {
    fn failure(&self) -> ImageFailure {
        match self {
            DecodeError::Missing { .. } => ImageFailure::Missing,
            DecodeError::Read { .. }
            | DecodeError::Decode { .. }
            | DecodeError::Panicked { .. } => ImageFailure::Unreadable,
        }
    }
}

/// Runs on a worker thread. A failure is logged here, once, and handed back as a state.
pub(super) fn decode(ctx: &egui::Context, job: DecodeJob) -> Decoded {
    let DecodeJob { path, max_side } = job;
    let loaded = catch_unwind(AssertUnwindSafe(|| load(ctx, &path, max_side)))
        .unwrap_or_else(|_| Err(DecodeError::Panicked { path: path.clone() }));
    let outcome = loaded.map_err(|error| {
        let cause = error.source().map(tracing::field::display);
        tracing::warn!(%error, cause, "a picture cannot be shown");
        error.failure()
    });
    Decoded { path, outcome }
}

fn load(ctx: &egui::Context, path: &Path, max_side: u32) -> Result<Loaded, DecodeError> {
    let file = File::open(path).map_err(|source| match source.kind() {
        io::ErrorKind::NotFound => DecodeError::Missing {
            path: path.to_path_buf(),
        },
        _ => DecodeError::Read {
            path: path.to_path_buf(),
            source,
        },
    })?;
    let mut reader = image::ImageReader::new(BufReader::new(file))
        .with_guessed_format()
        .map_err(|source| DecodeError::Read {
            path: path.to_path_buf(),
            source,
        })?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_FILE_SIDE);
    limits.max_image_height = Some(MAX_FILE_SIDE);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    reader.limits(limits);
    let decoded = reader.decode().map_err(|source| DecodeError::Decode {
        path: path.to_path_buf(),
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
            let name = format!("image:{}#{level}", path.display());
            ctx.load_texture(name, colors, egui::TextureOptions::LINEAR)
        })
        .collect();
    Ok(Loaded { size, textures })
}
