//! Where a figure sits on its page, and the picture cut out of the page for it.

use std::io::Read;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::checks::piece_strings;
use crate::content::{PAGE_IMAGE_FILE, PAGE_PDF_FILE, WholePageFigure};
use crate::poppler::{self, PageText, PopplerError, TextLine};
use crate::transcribe::{TranscribedPage, TranscribedPiece};

mod refine;

/// A rectangle in the picture of the page, in thousandths of the picture's width and height.
///
/// The numbers are whole and are not limited by the schema, so that any number the model writes
/// reaches the reply check instead of failing the call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
// Do not remove: without it the doc comment above is sent to the model as the description.
#[schemars(
    description = "A rectangle in the picture of the page, in thousandths of the picture's width and height counted from the picture's top-left corner."
)]
pub struct PageBox {
    #[schemars(
        description = "The left edge: thousandths of the picture's width from the picture's left edge, 0 to 1000."
    )]
    pub left: i32,
    #[schemars(
        description = "The top edge: thousandths of the picture's height from the picture's top edge, 0 to 1000."
    )]
    pub top: i32,
    #[schemars(
        description = "The right edge: thousandths of the picture's width from the picture's left edge, 0 to 1000, more than left."
    )]
    pub right: i32,
    #[schemars(
        description = "The bottom edge: thousandths of the picture's height from the picture's top edge, 0 to 1000, more than top."
    )]
    pub bottom: i32,
}

/// Added to every side of the rectangle that is cut, in thousandths of the page. It is under the
/// narrowest gap measured between a figure and the text next to it, and it forgives an edge a
/// little too tight.
const FIGURE_PADDING: i32 = 15;
/// A rectangle with a side shorter than this, in thousandths of the page, cannot be a figure. It
/// catches a reply written as fractions of the page.
pub(crate) const MIN_FIGURE_SIDE: i32 = 20;

impl PageBox {
    /// What makes the rectangle unusable, or `None`. The checks run in a fixed order and the
    /// first that fails is returned.
    pub(crate) fn problem(&self) -> Option<&'static str> {
        let sides = [self.left, self.top, self.right, self.bottom];
        if sides.iter().any(|side| !(0..=1000).contains(side)) {
            return Some("a side is not between 0 and 1000");
        }
        if self.left >= self.right {
            return Some("left is not less than right");
        }
        if self.top >= self.bottom {
            return Some("top is not less than bottom");
        }
        if self.right <= 100 && self.bottom <= 100 {
            return Some(
                "every number is 100 or less, which reads as percentages or fractions, not thousandths",
            );
        }
        if self.right - self.left < MIN_FIGURE_SIDE || self.bottom - self.top < MIN_FIGURE_SIDE {
            return Some("it is too small to be a figure");
        }
        None
    }

    /// Grown by [`FIGURE_PADDING`] on every side and kept inside the page.
    pub(crate) fn padded(&self) -> PageBox {
        PageBox {
            left: (self.left - FIGURE_PADDING).max(0),
            top: (self.top - FIGURE_PADDING).max(0),
            right: (self.right + FIGURE_PADDING).min(1000),
            bottom: (self.bottom + FIGURE_PADDING).min(1000),
        }
    }
}

/// What a figure's picture shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImageShows {
    /// The figure, cut out of the page.
    Figure,
    /// The whole page, because the figure could not be cut out.
    WholePage,
}

/// The picture saved for a figure, as `page.json` records it.
// SMELL: `cut`, `holds_body_text` and `unchecked` mean something only when `shows` is `figure`,
// but nothing stops a whole-page picture from being saved with them set. The keys are already in
// saved pages, so they have to stay as they are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FigureImage {
    /// A file name in the page's folder.
    pub file: String,
    pub shows: ImageShows,
    /// The rectangle the picture was drawn from, padding included. `None` when `shows` is
    /// `whole-page`.
    #[serde(default)]
    pub cut: Option<PageBox>,
    /// A line of another piece of the page is still wholly inside the picture, so the picture may
    /// show body text.
    #[serde(default)]
    pub holds_body_text: bool,
    /// Nothing on the page could be used to check the rectangle the model gave: none of the
    /// figure's own printed lines was found at the rectangle in the page's text layer, the layer
    /// could not be read or the page is turned, or the refined rectangle came out unusable. The
    /// picture is then cut from the model's rectangle with its padding, as given, and may hold a
    /// paragraph or miss part of the figure. False whenever at least one of the figure's own
    /// lines was found there, even when the rectangle then needed no change.
    #[serde(default)]
    pub unchecked: bool,
}

/// The pictures cut for the figures of one page.
#[derive(Debug, Default)]
pub(crate) struct CutFigures {
    /// By piece number, lowest first.
    pub images: Vec<(u32, FigureImage)>,
    /// The figures that got the whole page, with why.
    pub whole_page: Vec<WholePageFigure>,
}

impl CutFigures {
    fn fall_back(&mut self, piece: u32, why: String) {
        let image = FigureImage {
            file: PAGE_IMAGE_FILE.to_owned(),
            shows: ImageShows::WholePage,
            cut: None,
            holds_body_text: false,
            unchecked: false,
        };
        self.images.push((piece, image));
        self.whole_page.push(WholePageFigure { piece, why });
    }
}

/// What the page offers for cutting: how big its picture is and where its text lines sit.
struct Sheet {
    picture_size: (u32, u32),
    lines: Vec<TextLine>,
}

/// Cuts a picture for every figure of `page` out of `page.pdf` in `page_folder`, beside
/// `page.png`. It never fails: a figure that cannot be cut gets the whole page as its picture,
/// and the reason is returned, so a bad rectangle or a failed tool never stops a chapter.
pub(crate) async fn cut_figures(page_folder: &Path, page: &TranscribedPage) -> CutFigures {
    let mut cut = CutFigures::default();
    let mut usable = Vec::new();
    for (index, piece) in page.pieces.iter().enumerate() {
        if let TranscribedPiece::Figure { number, bounds, .. } = piece {
            match bounds.problem() {
                Some(seen) => cut.fall_back(*number, seen.to_owned()),
                None => usable.push((index, *number, *bounds)),
            }
        }
    }
    if !usable.is_empty() {
        match read_sheet(page_folder).await {
            Ok(sheet) => {
                for (index, number, bounds) in usable {
                    let (own, other) = strings_around(page, index);
                    let rectangle = refine::cut_rectangle(bounds, &own, &other, &sheet.lines);
                    match draw(page_folder, number, &sheet, &rectangle).await {
                        Ok(image) => cut.images.push((number, image)),
                        Err(error) => cut.fall_back(number, without_folder(&error, page_folder)),
                    }
                }
            }
            Err(error) => {
                let why = format!("could not read the size of {PAGE_IMAGE_FILE}: {error}");
                for (_, number, _) in usable {
                    cut.fall_back(number, why.clone());
                }
            }
        }
    }
    cut.images.sort_by_key(|(number, _)| *number);
    cut.whole_page.sort_by_key(|figure| figure.piece);
    cut
}

/// Fails only when the size of the page's picture cannot be read.
async fn read_sheet(page_folder: &Path) -> std::io::Result<Sheet> {
    let picture_size = png_size(&page_folder.join(PAGE_IMAGE_FILE))?;
    // Without lines the model's rectangle is cut as it is, padded.
    // SMELL: why the lines could not be used (the tool failed, or the page is turned) is not
    // saved. The figure is only marked unchecked, and the summary lists its page.
    let lines = match poppler::text_lines(&page_folder.join(PAGE_PDF_FILE)).await {
        Ok(text) if is_drawn_the_same_way(&text, picture_size) => text.lines,
        _ => Vec::new(),
    };
    Ok(Sheet {
        picture_size,
        lines,
    })
}

/// False for a page that is stored turned on its side. `pdftotext` measures such a page's line
/// boxes on the turned page but prints the size of the page before it was turned, so thousandths
/// worked out from that size are wrong. The boxes are trusted only when the printed size has the
/// same shape as the picture drawn for the page.
fn is_drawn_the_same_way(text: &PageText, (picture_width, picture_height): (u32, u32)) -> bool {
    // Drawing a page to whole pixels changes its shape by far less than this.
    const SAME_SHAPE_WITHIN: f64 = 0.01;
    let printed = text.width / text.height;
    let drawn = f64::from(picture_width) / f64::from(picture_height);
    (printed / drawn - 1.0).abs() <= SAME_SHAPE_WITHIN
}

/// The strings the figure at `index` printed, and the strings the rest of the page copied from
/// the page.
fn strings_around(page: &TranscribedPage, index: usize) -> (Vec<String>, Vec<String>) {
    let mut other: Vec<String> = [&page.printed_page_number, &page.running_header]
        .into_iter()
        .flatten()
        .cloned()
        .collect();
    for (at, piece) in page.pieces.iter().enumerate() {
        if at != index {
            other.extend(printed_strings(piece));
        }
    }
    (printed_strings(&page.pieces[index]), other)
}

/// What a piece printed. A figure's label and caption are also given as one string, because they
/// are printed on one line.
fn printed_strings(piece: &TranscribedPiece) -> Vec<String> {
    let mut strings: Vec<String> = piece_strings(piece)
        .into_iter()
        .map(str::to_owned)
        .collect();
    if let TranscribedPiece::Figure { label, caption, .. } = piece {
        let on_one_line: Vec<&str> = [label, caption]
            .into_iter()
            .flat_map(|part| part.as_deref())
            .collect();
        strings.push(on_one_line.join(" "));
    }
    strings
}

async fn draw(
    page_folder: &Path,
    number: u32,
    sheet: &Sheet,
    cut: &refine::FigureCut,
) -> Result<FigureImage, PopplerError> {
    let file = figure_image_file_name(number);
    let destination = page_folder.join(&file);
    let drawn = poppler::render_region(
        &page_folder.join(PAGE_PDF_FILE),
        sheet.picture_size,
        cut.area,
        &destination,
    )
    .await;
    if let Err(error) = drawn {
        // The tool may have written part of the file, or nothing, so a missing file is fine.
        std::fs::remove_file(&destination).ok();
        return Err(error);
    }
    Ok(FigureImage {
        file,
        shows: ImageShows::Figure,
        cut: Some(cut.area),
        holds_body_text: cut.holds_body_text,
        unchecked: cut.unchecked,
    })
}

fn figure_image_file_name(number: u32) -> String {
    format!("{number:02}-figure.png")
}

/// A saved reason must not hold the working folder's path, which no longer exists once the page
/// is finished.
// SMELL: the path is taken out by matching text. A tool that prints the folder spelled another
// way, such as through a symlink, would leave the working path in the saved reason.
fn without_folder(error: &PopplerError, page_folder: &Path) -> String {
    error
        .to_string()
        .replace(&format!("{}/", page_folder.display()), "")
}

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// The width and height in pixels of a PNG file, read from its first 24 bytes.
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
