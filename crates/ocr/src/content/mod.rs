//! The saved shape of a converted chapter and of a picture that stands alone: folder and file
//! names, and the `chapter.json`, `page.json` and `image.json` files that index them.

mod catalogue;
mod conversion;
mod figure_image;
mod index;
mod piece;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub use catalogue::{Catalogue, ChapterEntry, ImageEntry, saved_images};
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
        "a book chapter's file name must look like {CHAPTER_FILE_PATTERN} (for example chapter-1-financial-contracts.pdf), but it is {name:?}"
    )]
    BadFileName { name: String },

    #[error("media title {title:?} has no letters or digits to name its folder after")]
    EmptyMediaFolderName { title: String },

    #[error("document title {title:?} has no letters or digits to name its folder after")]
    EmptyDocumentFolderName { title: String },

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

/// How a document of a media is named: a chapter of a book, or a document with a title of its
/// own, such as a paper, a document of another media, or a part of a book that is not a chapter.
/// The order puts chapters first, by number, and then titled documents by title.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DocumentName {
    Chapter { number: u32, name: String },
    Title(String),
}

/// A document together with the title of the media it belongs to: what names its folder.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MediaDocument {
    pub media_title: String,
    pub name: DocumentName,
}

impl MediaDocument {
    /// A part of a book that is not a chapter, such as its index. The stored title starts with
    /// the book's, so the parts of two books that share a name stay apart.
    pub fn book_part(media_title: &str, part_title: &str) -> MediaDocument {
        MediaDocument {
            media_title: media_title.to_owned(),
            name: DocumentName::Title(format!("{}, {}", media_title.trim(), part_title.trim())),
        }
    }

    /// `<media folder>/<document folder>`, where a chapter's folder is `chapter-<number>` and a
    /// titled document's is its own title in lower case with dashes. The book is left out of a
    /// part's title, so the index of a book is in `<media folder>/index`.
    ///
    /// # Errors
    /// - [`ContentError::EmptyMediaFolderName`] for a media title with no letters or digits
    /// - [`ContentError::EmptyDocumentFolderName`] for a title with no letters or digits
    pub(crate) fn folder(&self) -> Result<PathBuf, ContentError> {
        let media_folder = media_folder_name(&self.media_title)?;
        let document_folder = match &self.name {
            DocumentName::Chapter { number, .. } => format!("chapter-{number}"),
            DocumentName::Title(title) => {
                let own = own_title(&self.media_title, title);
                slug(own).ok_or_else(|| ContentError::EmptyDocumentFolderName {
                    title: own.to_owned(),
                })?
            }
        };
        Ok(Path::new(&media_folder).join(document_folder))
    }
}

/// The title without `<media title>, ` in front, whatever the capitals of the media title. A
/// title that does not start that way is given whole.
pub fn own_title<'a>(media_title: &str, title: &'a str) -> &'a str {
    let media = media_title.trim();
    title
        .split_at_checked(media.len())
        .filter(|(start, _)| start.to_lowercase() == media.to_lowercase())
        .and_then(|(_, rest)| rest.strip_prefix(", "))
        .unwrap_or(title)
}

/// The file name at the end of `path` as text, or empty text when there is none.
pub fn file_name_text(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Reads the number and the name of a book chapter from `chapter-<number>-<name>.pdf`.
///
/// # Errors
/// [`ContentError::BadFileName`] for a name of any other shape.
pub fn parse_chapter_file_name(file_name: &str) -> Result<DocumentName, ContentError> {
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
    Ok(DocumentName::Chapter {
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

/// # Errors
/// [`ContentError::EmptyMediaFolderName`] for a title with no letters or digits.
pub fn media_folder_name(media_title: &str) -> Result<String, ContentError> {
    slug(media_title).ok_or_else(|| ContentError::EmptyMediaFolderName {
        title: media_title.to_owned(),
    })
}

/// The text in lower case, with one dash for each run of characters that are not letters or
/// digits. `None` when no letter or digit is left.
fn slug(text: &str) -> Option<String> {
    let mut folder = String::new();
    for character in text.to_lowercase().chars() {
        if character.is_alphanumeric() {
            folder.push(character);
        } else if !folder.is_empty() && !folder.ends_with('-') {
            folder.push('-');
        }
    }
    if folder.ends_with('-') {
        folder.pop();
    }
    (!folder.is_empty()).then_some(folder)
}

/// Page positions start at 1.
pub fn page_folder_name(page_position: u32) -> String {
    format!("page-num-{page_position}")
}

/// The path starts at the output root. The same bytes always land in the same folder, whatever
/// the file was called.
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

pub const CHAPTER_INDEX_FILE: &str = "chapter.json";
pub const PAGE_INDEX_FILE: &str = "page.json";
pub const PAGE_PDF_FILE: &str = "page.pdf";
pub const PAGE_IMAGE_FILE: &str = "page.png";
/// The page's text as Poppler read it, saved unchanged. It can hold misread words.
pub const TEXT_LAYER_FILE: &str = "text-layer.txt";
pub const IMAGE_INDEX_FILE: &str = "image.json";
/// The explanation of a picture that stands alone, then the words printed on it.
pub const IMAGE_EXPLANATION_FILE: &str = "figure.md";
/// The second of a page's two rejected replies, left in its working folder to be looked at.
pub const REJECTED_REPLY_FILE: &str = "rejected-reply.json";
