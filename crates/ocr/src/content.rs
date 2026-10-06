//! The saved shape of a converted chapter: folder and file names, and the `chapter.json` and
//! `page.json` files that index it. Everything that reads or writes `content/` goes through here.

mod conversion;
mod index;
mod piece;

use std::path::PathBuf;

pub use crate::figure::{FigureImage, ImageShows, PageBox};
pub use conversion::{
    CallRecord, CallStep, Checks, Conversion, MathCheck, Route, RouteReason, WholePageFigure,
    WordMatch,
};
pub use index::{ChapterIndex, PageIndex};
pub use piece::{Cite, CiteKind, PieceDetail, PieceEntry, Relationship, RelationshipKind, Symbol};

/// Bumped whenever a saved shape changes in a way an older reader would misread.
pub const FORMAT_VERSION: u32 = 1;

const CHAPTER_FILE_PATTERN: &str = "chapter-<number>-<name>.pdf";

/// Why a name could not be turned into a folder, or an index file could not be read or written.
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

/// The chapter number and name a chapter file's name carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChapterFileName {
    pub number: u32,
    pub name: String,
}

/// Reads `chapter-<number>-<name>.pdf`: the number is all digits and the name has at least one
/// word. The name is the words between the hyphens, each with a capital letter.
///
/// # Errors
/// - [`ContentError::BadFileName`] for any other name
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
///
/// # Errors
/// - [`ContentError::EmptyBookFolderName`] if the title has no letters or digits
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

/// The folder of one chapter, such as `chapter-7`.
pub fn chapter_folder_name(chapter_number: u32) -> String {
    format!("chapter-{chapter_number}")
}

/// The folder of one finished page, such as `page-num-3`. Positions start at 1.
pub fn page_folder_name(page_position: u32) -> String {
    format!("page-num-{page_position}")
}

/// Where a page is built. The folder is renamed to [`page_folder_name`] when it is complete.
pub fn partial_page_folder_name(page_position: u32) -> String {
    format!("page-num-{page_position}.partial")
}

/// True for the name of a folder a page was being built in, whatever its position.
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
/// A picture of the whole page.
pub const PAGE_IMAGE_FILE: &str = "page.png";
/// The page's text as Poppler read it, saved unchanged. It can hold misread words.
pub const TEXT_LAYER_FILE: &str = "text-layer.txt";
/// The second of a page's two rejected replies, left in its working folder to be looked at.
pub const REJECTED_REPLY_FILE: &str = "rejected-reply.json";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_name_gives_chapter_number_name_and_folders() {
        let reserved = parse_chapter_file_name("chapter-18-the-black-scholes-model.pdf").unwrap();
        assert_eq!(reserved.number, 18);
        assert_eq!(reserved.name, "The Black Scholes Model");
        assert_eq!(chapter_folder_name(reserved.number), "chapter-18");

        assert_eq!(
            book_folder_name("Option Volatility and Pricing").unwrap(),
            "option-volatility-and-pricing"
        );
        assert_eq!(
            book_folder_name("  C++: The *Book*! ").unwrap(),
            "c-the-book"
        );

        for refused in ["notes.pdf", "chapter-x-name.pdf", "chapter-18.pdf"] {
            let error = parse_chapter_file_name(refused).unwrap_err();
            let message = error.to_string();
            assert!(message.contains(CHAPTER_FILE_PATTERN), "{message}");
            assert!(message.contains(refused), "{message}");
        }
        assert!(matches!(
            book_folder_name("?! --"),
            Err(ContentError::EmptyBookFolderName { .. })
        ));
    }
}
