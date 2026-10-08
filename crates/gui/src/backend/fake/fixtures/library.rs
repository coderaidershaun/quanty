//! The catalogue and the pages that are read from the committed sample chapters, built as the
//! live backend builds them.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use uuid::Uuid;

use crate::contract::{
    Book, Catalogue, ChapterLabel, DocId, Document, ImageRef, ItemCounts, PageBox, PagePiece,
    PageView, PieceKind,
};

/// The three sample chapters: the number of the document, its book folder and its chapter folder.
const CHAPTERS: [(u128, &str, &str); 3] = [
    (1, "quanty-sample-notes", "chapter-1"),
    (2, "quanty-sample-notes", "chapter-2"),
    (3, "option-volatility-and-pricing", "chapter-1"),
];

/// The author and the tags each sample document carries, in the order of `CHAPTERS`. The length
/// is that of `CHAPTERS`, so a chapter with no row here does not compile.
const LABELS: [(Option<&str>, &[&str]); CHAPTERS.len()] = [
    (Some("Quanty Team"), &["notes", "options"]),
    (Some("Quanty Team"), &["notes", "black-scholes"]),
    (None, &["book", "volatility"]),
];

#[derive(Debug, thiserror::Error)]
pub(in crate::backend::fake) enum SampleError {
    #[error("no sample chapter has the document id {doc}")]
    UnknownDocument { doc: Uuid },
    #[error("the sample chapter has {page_count} pages, so page {page} is not in it")]
    NoSuchPage { page: u32, page_count: u32 },
    #[error("page {page} of the sample document {doc} has no piece {piece}")]
    NoSuchPiece { doc: Uuid, page: u32, piece: u32 },
    #[error("could not read {}: {source}", path.display())]
    Unreadable {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{} is not a sample file of the expected shape: {source}", path.display())]
    Malformed {
        path: PathBuf,
        source: serde_json::Error,
    },
}

// SMELL: this reads the saved chapter files a second time, apart from the real reader, because
// the fake backend may not name a backend crate. A change to the saved format must be made here
// too, and the test of every sample page fails when it is not.
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ChapterFile {
    book_title: String,
    chapter_number: u32,
    chapter_name: String,
    page_count: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct PageFile {
    printed_page_number: Option<String>,
    pieces: Vec<PieceFile>,
}

#[derive(Deserialize)]
struct PieceFile {
    number: u32,
    file: String,
    #[serde(flatten)]
    detail: Detail,
}

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "kebab-case"
)]
enum Detail {
    Heading {
        rank: u8,
        #[serde(default)]
        printed_number: Option<String>,
    },
    Text,
    Formula {
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        name: Option<String>,
    },
    Figure {
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        caption: Option<String>,
        #[serde(default)]
        image: Option<FigureImage>,
    },
    Table {
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        caption: Option<String>,
    },
    Footnote {
        #[serde(default)]
        marker: Option<String>,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct FigureImage {
    file: String,
    shows: Shows,
    #[serde(default)]
    cut: Option<Cut>,
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum Shows {
    Figure,
    WholePage,
}

#[derive(Deserialize)]
struct Cut {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

pub(in crate::backend::fake) fn find_samples(near: &Path) -> Option<PathBuf> {
    near.ancestors()
        .map(|folder| folder.join("samples").join("content"))
        .find(|candidate| candidate.is_dir())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, SampleError> {
    let text = std::fs::read_to_string(path).map_err(|source| SampleError::Unreadable {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| SampleError::Malformed {
        path: path.to_path_buf(),
        source,
    })
}

fn read_piece(path: &Path) -> Result<String, SampleError> {
    let mut text = std::fs::read_to_string(path).map_err(|source| SampleError::Unreadable {
        path: path.to_path_buf(),
        source,
    })?;
    if text.ends_with('\n') {
        text.pop();
    }
    Ok(text)
}

fn chapter_folder(samples: &Path, doc: DocId) -> Result<PathBuf, SampleError> {
    CHAPTERS
        .iter()
        .find(|(number, ..)| doc.0 == Uuid::from_u128(*number))
        .map(|(_, book, chapter)| samples.join(book).join(chapter))
        .ok_or(SampleError::UnknownDocument { doc: doc.0 })
}

pub(super) fn title_of(book: &str, number: u32, name: &str) -> String {
    format!("{book}, chapter {number}: {name}")
}

/// # Errors
/// When a `chapter.json` or a `page.json` cannot be read.
pub(in crate::backend::fake) fn catalogue(samples: &Path) -> Result<Catalogue, SampleError> {
    let mut documents: Vec<(String, u32, Document)> = Vec::new();
    for ((number, ..), (author, tags)) in CHAPTERS.iter().zip(LABELS) {
        let doc = DocId(Uuid::from_u128(*number));
        let folder = chapter_folder(samples, doc)?;
        let chapter: ChapterFile = read_json(&folder.join("chapter.json"))?;
        let mut items = ItemCounts::default();
        for position in 1..=chapter.page_count {
            let page = read_page_file(&folder, position)?;
            for piece in &page.pieces {
                match piece.detail {
                    Detail::Text | Detail::Footnote { .. } => items.chunks += 1,
                    Detail::Formula { .. } => items.formulas += 1,
                    Detail::Figure { .. } => items.figures += 1,
                    Detail::Table { .. } => items.tables += 1,
                    Detail::Heading { .. } => {}
                }
            }
        }
        let total = items.chunks + items.formulas + items.figures + items.tables;
        documents.push((
            chapter.book_title.clone(),
            chapter.chapter_number,
            Document {
                id: doc,
                title: title_of(
                    &chapter.book_title,
                    chapter.chapter_number,
                    &chapter.chapter_name,
                ),
                chapter: Some(ChapterLabel {
                    number: chapter.chapter_number,
                    name: chapter.chapter_name,
                }),
                author: author.map(str::to_owned),
                tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
                pages: Some(chapter.page_count),
                items,
                ingested_items: Some(total),
                folder: Some(folder),
            },
        ));
    }
    documents.sort_by_key(|(book, number, _)| (book.to_lowercase(), *number));
    let mut books: Vec<Book> = Vec::new();
    for (title, _, document) in documents {
        match books.last_mut() {
            Some(book) if book.title.as_deref() == Some(title.as_str()) => {
                book.chapters.push(document);
            }
            _ => books.push(Book {
                title: Some(title),
                chapters: vec![document],
            }),
        }
    }
    Ok(Catalogue { books })
}

fn read_page_file(folder: &Path, position: u32) -> Result<PageFile, SampleError> {
    read_json(
        &folder
            .join(format!("page-num-{position}"))
            .join("page.json"),
    )
}

fn picture(path: PathBuf) -> Option<ImageRef> {
    path.is_file().then_some(ImageRef { path })
}

fn page_picture(folder: &Path, position: u32) -> Option<ImageRef> {
    picture(folder.join(format!("page-num-{position}")).join("page.png"))
}

/// The cut a figure's picture was made from, only when the picture shows the figure and the
/// rectangle fits the page.
fn usable_cut(image: &FigureImage) -> Option<PageBox> {
    let cut = image
        .cut
        .as_ref()
        .filter(|_| image.shows == Shows::Figure)?;
    let sides = [cut.left, cut.top, cut.right, cut.bottom];
    let fits = sides.iter().all(|side| (0..=1000).contains(side))
        && cut.left < cut.right
        && cut.top < cut.bottom;
    fits.then(|| PageBox {
        left: u16::try_from(cut.left).unwrap_or_default(),
        top: u16::try_from(cut.top).unwrap_or_default(),
        right: u16::try_from(cut.right).unwrap_or_default(),
        bottom: u16::try_from(cut.bottom).unwrap_or_default(),
    })
}

fn piece_of(folder: &Path, position: u32, entry: PieceFile) -> Result<PagePiece, SampleError> {
    let page_folder = folder.join(format!("page-num-{position}"));
    let text = read_piece(&page_folder.join(&entry.file))?;
    let mut piece = PagePiece {
        number: entry.number,
        kind: PieceKind::Text,
        label: None,
        name: None,
        caption: None,
        text,
        image: None,
        cut: None,
    };
    match entry.detail {
        Detail::Heading {
            rank,
            printed_number,
        } => {
            piece.kind = PieceKind::Heading { rank };
            piece.label = printed_number;
        }
        Detail::Text => {}
        Detail::Formula { label, name } => {
            piece.kind = PieceKind::Formula;
            piece.label = label;
            piece.name = name;
        }
        Detail::Figure {
            label,
            caption,
            image,
        } => {
            piece.kind = PieceKind::Figure;
            piece.label = label;
            piece.caption = caption;
            // A figure with no saved picture shows the whole page.
            let file = image
                .as_ref()
                .map_or("page.png", |image| image.file.as_str());
            piece.image = picture(page_folder.join(file));
            piece.cut = image.as_ref().and_then(usable_cut);
        }
        Detail::Table { label, caption } => {
            piece.kind = PieceKind::Table;
            piece.label = label;
            piece.caption = caption;
        }
        Detail::Footnote { marker } => {
            piece.kind = PieceKind::Footnote;
            piece.label = marker;
        }
    }
    Ok(piece)
}

/// # Errors
/// [`SampleError::UnknownDocument`] for a document that is not a sample,
/// [`SampleError::NoSuchPage`] for a page outside the chapter, and the errors of reading a file.
pub(in crate::backend::fake) fn page(
    samples: &Path,
    doc: DocId,
    page: u32,
) -> Result<PageView, SampleError> {
    let folder = chapter_folder(samples, doc)?;
    let chapter: ChapterFile = read_json(&folder.join("chapter.json"))?;
    if page == 0 || page > chapter.page_count {
        return Err(SampleError::NoSuchPage {
            page,
            page_count: chapter.page_count,
        });
    }
    let file = read_page_file(&folder, page)?;
    let printed_page = file.printed_page_number.clone();
    let pieces = file
        .pieces
        .into_iter()
        .map(|entry| piece_of(&folder, page, entry))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PageView {
        doc,
        page,
        book: Some(chapter.book_title),
        chapter: Some(ChapterLabel {
            number: chapter.chapter_number,
            name: chapter.chapter_name,
        }),
        page_count: chapter.page_count,
        printed_page,
        image: page_picture(&folder, page),
        previous_image: (page > 1)
            .then(|| page_picture(&folder, page - 1))
            .flatten(),
        next_image: (page < chapter.page_count)
            .then(|| page_picture(&folder, page + 1))
            .flatten(),
        pieces,
    })
}
