//! Pictures read from files for a document, and the data address that puts a picture inside the
//! file that shows it.

use std::io::Cursor;
use std::path::Path;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use image::ImageFormat;

/// The kinds of picture a document takes from a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PictureKind {
    Png,
    Jpeg,
}

impl PictureKind {
    pub(crate) fn mime(self) -> &'static str {
        match self {
            PictureKind::Png => "image/png",
            PictureKind::Jpeg => "image/jpeg",
        }
    }
}

/// A PNG or JPEG file, as it is stored, with the size of its pixels.
pub(crate) struct Picture {
    pub(crate) bytes: Vec<u8>,
    pub(crate) kind: PictureKind,
    pub(crate) width_px: u32,
    pub(crate) height_px: u32,
}

impl Picture {
    /// `None` for a file that cannot be read, or that is neither a PNG nor a JPEG.
    pub(crate) fn read(path: &Path) -> Option<Picture> {
        let bytes = std::fs::read(path).ok()?;
        let kind = match image::guess_format(&bytes).ok()? {
            ImageFormat::Png => PictureKind::Png,
            ImageFormat::Jpeg => PictureKind::Jpeg,
            _ => return None,
        };
        let (width_px, height_px) = image::ImageReader::new(Cursor::new(&bytes))
            .with_guessed_format()
            .ok()?
            .into_dimensions()
            .ok()?;
        Some(Picture {
            bytes,
            kind,
            width_px,
            height_px,
        })
    }

    pub(crate) fn data_uri(&self) -> String {
        data_uri(self.kind.mime(), &self.bytes)
    }
}

/// The address that holds the bytes of a file inside the text that names it.
pub(crate) fn data_uri(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", STANDARD.encode(bytes))
}
