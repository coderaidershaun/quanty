//! What the page offers for cutting: the size of its picture and where its text lines sit.

use std::io::Read;
use std::path::Path;

use crate::content::{PAGE_IMAGE_FILE, PAGE_PDF_FILE};
use crate::convert::poppler::{self, PageText, TextLine};

pub(super) struct Sheet {
    pub(super) picture_size: (u32, u32),
    pub(super) lines: Vec<TextLine>,
}

/// Fails only when the size of the page's picture cannot be read.
pub(super) async fn read_sheet(page_folder: &Path) -> std::io::Result<Sheet> {
    let picture_size = png_size(&page_folder.join(PAGE_IMAGE_FILE))?;
    // Without lines the model's rectangle is cut as it is, padded.
    // SMELL: why the lines could not be used, a failed tool or a turned page, is lost, and the
    // figure is only marked unchecked. Keeping it needs a new key in the saved page.
    let lines = match poppler::text_lines(&page_folder.join(PAGE_PDF_FILE)).await {
        Ok(text) if is_drawn_the_same_way(&text, picture_size) => text.lines,
        _ => Vec::new(),
    };
    Ok(Sheet {
        picture_size,
        lines,
    })
}

/// False for a page stored turned on its side: `pdftotext` measures its line boxes on the turned
/// page but prints the size from before the turn, so thousandths worked out from that size are
/// wrong. The boxes are trusted only when the printed size has the same shape as the picture.
fn is_drawn_the_same_way(text: &PageText, (picture_width, picture_height): (u32, u32)) -> bool {
    // Drawing a page to whole pixels changes its shape by far less than this.
    const SAME_SHAPE_WITHIN: f64 = 0.01;
    let printed = text.width / text.height;
    let drawn = f64::from(picture_width) / f64::from(picture_height);
    (printed / drawn - 1.0).abs() <= SAME_SHAPE_WITHIN
}

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Reads only the first 24 bytes, which hold the signature and the size.
fn png_size(path: &Path) -> std::io::Result<(u32, u32)> {
    let mut header = [0u8; 24];
    std::fs::File::open(path)?.read_exact(&mut header)?;
    if header[..8] != PNG_SIGNATURE {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "not a PNG file",
        ));
    }
    let number = |at: usize| {
        u32::from_be_bytes([header[at], header[at + 1], header[at + 2], header[at + 3]])
    };
    Ok((number(16), number(20)))
}
