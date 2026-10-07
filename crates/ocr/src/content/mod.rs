//! The saved shape of a converted chapter and of a picture that stands alone: folder and file
//! names, and the `chapter.json`, `page.json` and `image.json` files that index them. It also
//! lists the chapters under a content folder. The converter and the reader both build on it, and
//! it uses neither.

mod catalogue;
mod conversion;
mod figure_image;
mod index;
mod piece;

use std::path::{Path, PathBuf};

pub use catalogue::{Catalogue, ChapterEntry};
pub use conversion::{
    CallRecord, CallStep, Checks, Conversion, MathCheck, PageCategories, Route, RouteReason,
    WholePageFigure, WordMatch,
};
pub(crate) use figure_image::figure_image_file_name;
pub use figure_image::{FigureImage, ImageShows, PageBox};
pub use index::{ChapterIndex, ImageIndex, PageIndex};
pub use piece::{Cite, CiteKind, PieceDetail, PieceEntry, Relationship, RelationshipKind, Symbol};

/// Bumped whenever a saved shape changes in a way an older reader would misread.
pub const FORMAT_VERSION: u32 = 1;

const CHAPTER_FILE_PATTERN: &str = "chapter-<number>-<name>.pdf";
/// The folder under the output root that holds every converted picture that stands alone.
const IMAGES_FOLDER: &str = "images";
/// How many hex digits of the picture's SHA-256 name its folder.
const IMAGE_FOLDER_DIGITS: usize = 16;

#[derive(thiserror::Error, Debug)]
pub enum ContentError {
    #[error(
        "chapter file name must look like {CHAPTER_FILE_PATTERN} (for example chapter-1-financial-contracts.pdf), but it is {name:?}"
    )]
    BadFileName { name: String },

    #[error("book title {title:?} has no letters or digits to name its folder after")]
    EmptyBookFolderName { title: String },

    #[error("could not read {}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{} is not valid json of the expected shape", path.display())]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("could not write {}", path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChapterFileName {
    pub number: u32,
    pub name: String,
}

/// Reads `chapter-<number>-<name>.pdf`: the number is all digits and the name has at least one
/// word. The name is the words between the hyphens, each with a capital letter.
pub fn parse_chapter_file_name(file_name: &str) -> Result<ChapterFileName, ContentError> {
    let bad_name = || ContentError::BadFileName {
        name: file_name.to_owned(),
    };
    let stem = file_name
        .strip_prefix("chapter-")
        .and_then(|rest| rest.strip_suffix(".pdf"))
        .ok_or_else(bad_name)?;
    let (digits, words) = stem.split_once('-').ok_or_else(bad_name)?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(bad_name());
    }
    let number = digits.parse().map_err(|_| bad_name())?;
    let words: Vec<String> = words
        .split('-')
        .filter(|word| !word.is_empty())
        .map(capitalise)
        .collect();
    if words.is_empty() {
        return Err(bad_name());
    }
    Ok(ChapterFileName {
        number,
        name: words.join(" "),
    })
}

fn capitalise(word: &str) -> String {
    let mut letters = word.chars();
    match letters.next() {
        Some(first) => first.to_uppercase().chain(letters).collect(),
        None => String::new(),
    }
}

/// The folder a book's chapters live in: the title in lower case, every run of characters that
/// are not letters or digits turned into one `-`, and no `-` at either end.
pub fn book_folder_name(book_title: &str) -> Result<String, ContentError> {
    let mut folder = String::new();
    for character in book_title.to_lowercase().chars() {
        if character.is_alphanumeric() {
            folder.push(character);
        } else if !folder.is_empty() && !folder.ends_with('-') {
            folder.push('-');
        }
    }
    if folder.ends_with('-') {
        folder.pop();
    }
    if folder.is_empty() {
        return Err(ContentError::EmptyBookFolderName {
            title: book_title.to_owned(),
        });
    }
    Ok(folder)
}

pub fn chapter_folder_name(chapter_number: u32) -> String {
    format!("chapter-{chapter_number}")
}

/// Page positions start at 1.
pub fn page_folder_name(page_position: u32) -> String {
    format!("page-num-{page_position}")
}

/// The folder of a picture that stands alone, as a path from the output root:
/// `images/<the start of the picture's SHA-256>`. The same bytes always land in the same folder,
/// whatever the file was called.
pub fn image_folder(source_sha256: &str) -> PathBuf {
    let digits = source_sha256
        .get(..IMAGE_FOLDER_DIGITS)
        .unwrap_or(source_sha256);
    Path::new(IMAGES_FOLDER).join(digits)
}

/// The name of the copy of a picture that stands alone, kept in its folder. The caller gives the
/// extension in lower case.
pub fn image_copy_file_name(extension: &str) -> String {
    format!("picture.{extension}")
}

/// Where a page is built. The folder is renamed to [`page_folder_name`] when it is complete.
pub fn partial_page_folder_name(page_position: u32) -> String {
    format!("page-num-{page_position}.partial")
}

pub fn is_partial_page_folder_name(folder_name: &str) -> bool {
    folder_name
        .strip_prefix("page-num-")
        .and_then(|rest| rest.strip_suffix(".partial"))
        .is_some()
}

/// Says which PDF a chapter was made from, how many pages it has and whether it is finished.
pub const CHAPTER_INDEX_FILE: &str = "chapter.json";
/// Lists a page's pieces in reading order, with how the page was made.
pub const PAGE_INDEX_FILE: &str = "page.json";
/// The one page cut out of the chapter PDF.
pub const PAGE_PDF_FILE: &str = "page.pdf";
pub const PAGE_IMAGE_FILE: &str = "page.png";
/// The page's text as Poppler read it, saved unchanged. It can hold misread words.
pub const TEXT_LAYER_FILE: &str = "text-layer.txt";
/// Says which picture a folder was made from and what was read from it. It is written last, so a
/// folder that has it was converted whole.
pub const IMAGE_INDEX_FILE: &str = "image.json";
/// The explanation of a picture that stands alone, then the words printed on it.
pub const IMAGE_EXPLANATION_FILE: &str = "figure.md";
/// The second of a page's two rejected replies, left in its working folder to be looked at.
pub const REJECTED_REPLY_FILE: &str = "rejected-reply.json";
