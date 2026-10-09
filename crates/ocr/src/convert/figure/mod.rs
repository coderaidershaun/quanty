//! The picture cut out of its page for each figure.

mod refine;
mod sheet;

use std::path::Path;

use super::checks::{piece_strings, words};
use super::poppler::{self, PopplerError};
use super::reply::{TranscribedPage, TranscribedPiece};
use super::usable_box::UsableBox;
use crate::content::{
    FigureImage, ImageShows, PAGE_IMAGE_FILE, PAGE_PDF_FILE, WholePageFigure,
    figure_image_file_name,
};

use refine::PrintedWords;
use sheet::{Sheet, read_sheet};

#[derive(Debug, Default)]
pub(super) struct CutFigures {
    /// By piece number, lowest first.
    pub images: Vec<(u32, FigureImage)>,
    pub whole_page: Vec<WholePageFigure>,
}

impl CutFigures {
    fn fall_back(&mut self, piece: u32, why: String) {
        let image = FigureImage {
            file: PAGE_IMAGE_FILE.to_owned(),
            shows: ImageShows::WholePage,
        };
        self.images.push((piece, image));
        self.whole_page.push(WholePageFigure { piece, why });
    }
}

/// Never fails: a figure that cannot be cut gets the whole page as its picture, with the reason
/// returned, so a bad rectangle or a failed tool never stops a chapter.
pub(super) async fn cut_figures(page_folder: &Path, page: &TranscribedPage) -> CutFigures {
    let mut cut = CutFigures::default();
    let mut usable = Vec::new();
    for (index, piece) in page.pieces.iter().enumerate() {
        if let TranscribedPiece::Figure { number, bounds, .. } = piece {
            match UsableBox::new(*bounds) {
                Ok(bounds) => usable.push((index, *number, bounds)),
                Err(seen) => cut.fall_back(*number, seen.to_owned()),
            }
        }
    }
    if !usable.is_empty() {
        match read_sheet(page_folder).await {
            Ok(sheet) => {
                for (index, number, bounds) in usable {
                    let printed = words_around(page, index);
                    let rectangle = refine::cut_rectangle(bounds, &printed, &sheet.lines);
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

fn words_around(page: &TranscribedPage, index: usize) -> PrintedWords {
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
    let words_of = |strings: Vec<String>| strings.iter().map(|text| words(text)).collect();
    PrintedWords {
        own: words_of(printed_strings(&page.pieces[index])),
        other: words_of(other),
    }
}

/// A figure's label and caption are also given as one string, because they are printed on one
/// line.
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
        shows: ImageShows::Figure {
            cut: Some(cut.area.page_box()),
            holds_body_text: cut.holds_body_text,
            unchecked: cut.unchecked,
        },
    })
}

/// A saved reason must not hold the working folder's path, which no longer exists once the page
/// is finished.
// SMELL: the folder is taken out by matching text, so a tool that prints it spelled another way
// leaves the working path in the saved reason. Only running the tools from inside the page
// folder would end this, and the tool runner has no such option.
fn without_folder(error: &PopplerError, page_folder: &Path) -> String {
    error
        .to_string()
        .replace(&format!("{}/", page_folder.display()), "")
}
