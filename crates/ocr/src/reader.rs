//! Reads a converted chapter back as one list of pieces in reading order, each with the section
//! it sits under and the relationships it takes part in.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::content::{
    CHAPTER_INDEX_FILE, ChapterIndex, ContentError, FORMAT_VERSION, ImageShows, PAGE_IMAGE_FILE,
    PAGE_INDEX_FILE, PageIndex, PieceDetail, PieceEntry, RelationshipKind, page_folder_name,
};

#[derive(thiserror::Error, Debug)]
pub enum ReadChapterError {
    #[error(transparent)]
    Content(#[from] ContentError),

    #[error("the chapter in {} is not finished; convert it again to complete it", folder.display())]
    NotFinished { folder: PathBuf },

    #[error(
        "{} is saved at format version {found}, but this reader understands version {expected}",
        path.display()
    )]
    FormatVersion {
        path: PathBuf,
        found: u32,
        expected: u32,
    },

    #[error("could not read the piece file {}", path.display())]
    PieceFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error(
        "a relationship in {} names piece {number}, which that page does not have",
        page_folder.display()
    )]
    UnknownPiece { page_folder: PathBuf, number: u32 },
}

/// Where a piece sits in the chapter: its page position and its number on that page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PieceId {
    pub page: u32,
    pub number: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionHeading {
    pub rank: u8,
    pub printed_number: Option<String>,
    pub text: String,
}

/// A link between two pieces, which may be on neighbouring pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PieceRelationship {
    pub kind: RelationshipKind,
    pub from: PieceId,
    pub to: PieceId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FigurePicture {
    /// Built from the folder the caller passed, so it is absolute when that was. The file is not
    /// checked to exist.
    pub path: PathBuf,
    /// Whether `path` is the figure cut out of its page, or the whole page because that failed.
    pub shows: ImageShows,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChapterPiece {
    pub id: PieceId,
    pub printed_page_number: Option<String>,
    /// The headings it sits under, outermost first. Empty before the first heading.
    pub section: Vec<SectionHeading>,
    /// The piece file, without its closing newline.
    pub content: String,
    pub file: PathBuf,
    /// Where `page.png` of its page is, or would be.
    pub page_image: PathBuf,
    /// `None` unless the piece is a figure. A figure whose `page.json` names no image, such as
    /// one converted before figures were cut, gets the whole page.
    pub figure_image: Option<FigurePicture>,
    pub detail: PieceDetail,
    /// True only on the first text piece of a page that begins partway through a sentence.
    ///
    /// The two flags are not symmetric. Rejoin a paragraph across a page break only when
    /// `ends_mid_sentence` is set on the earlier piece and this flag is set on the later one.
    pub starts_mid_sentence: bool,
    /// True only on the last text piece of a page that ends partway through a sentence.
    ///
    /// A page that ends with "… we obtain" before a page that opens with the formula sets only
    /// this flag. An `introduces` relationship links the two instead.
    pub ends_mid_sentence: bool,
    /// Each relationship is listed on both of the pieces it joins.
    pub relationships: Vec<PieceRelationship>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chapter {
    pub index: ChapterIndex,
    /// In reading order: by page, then by number on the page.
    pub pieces: Vec<ChapterPiece>,
}

impl Chapter {
    pub fn piece(&self, id: PieceId) -> Option<&ChapterPiece> {
        self.pieces.iter().find(|piece| piece.id == id)
    }
}

/// Reads the finished chapter in `chapter_folder`.
///
/// Only `chapter.json`, each `page.json` and the piece files are needed, so a chapter written by
/// hand can be read too. A formula printed first on its page has its lead-in on the page before;
/// that `introduces` relationship is worked out here, because saved relationships stay on one
/// page.
pub fn read_chapter(chapter_folder: &Path) -> Result<Chapter, ReadChapterError> {
    let index = ChapterIndex::read(chapter_folder)?;
    if !index.finished {
        return Err(ReadChapterError::NotFinished {
            folder: chapter_folder.to_path_buf(),
        });
    }
    check_version(
        &chapter_folder.join(CHAPTER_INDEX_FILE),
        index.format_version,
    )?;

    let mut pieces = Vec::new();
    let mut pages = Vec::new();
    // SMELL: a heading's rank is judged from one page at a time, so two pages can disagree about
    // the same kind of heading and the sections of the pieces after it come out wrong. Nothing
    // here repairs that.
    let mut open_sections: Vec<SectionHeading> = Vec::new();
    for position in 1..=index.page_count {
        let page_folder = chapter_folder.join(page_folder_name(position));
        let page = PageIndex::read(&page_folder)?;
        check_version(&page_folder.join(PAGE_INDEX_FILE), page.format_version)?;
        let first_piece_of_page = pieces.len();
        for entry in &page.pieces {
            let file = page_folder.join(&entry.file);
            let content = read_piece_file(&file)?;
            let section = place_in_sections(&mut open_sections, entry, &content);
            pieces.push(ChapterPiece {
                id: PieceId {
                    page: position,
                    number: entry.number,
                },
                printed_page_number: page.printed_page_number.clone(),
                section,
                content,
                file,
                page_image: page_folder.join(PAGE_IMAGE_FILE),
                figure_image: figure_picture(&page_folder, &entry.detail),
                detail: entry.detail.clone(),
                starts_mid_sentence: false,
                ends_mid_sentence: false,
                relationships: Vec::new(),
            });
        }
        mark_mid_sentence_pieces(&mut pieces[first_piece_of_page..], &page);
        pages.push((page_folder, page));
    }

    attach_relationships(&mut pieces, collect_relationships(&pages)?);
    Ok(Chapter { index, pieces })
}

fn figure_picture(page_folder: &Path, detail: &PieceDetail) -> Option<FigurePicture> {
    let PieceDetail::Figure { image, .. } = detail else {
        return None;
    };
    Some(match image {
        Some(image) => FigurePicture {
            path: page_folder.join(&image.file),
            shows: image.shows,
        },
        None => FigurePicture {
            path: page_folder.join(PAGE_IMAGE_FILE),
            shows: ImageShows::WholePage,
        },
    })
}

fn attach_relationships(pieces: &mut [ChapterPiece], relationships: Vec<PieceRelationship>) {
    let positions: HashMap<PieceId, usize> = pieces
        .iter()
        .enumerate()
        .map(|(position, piece)| (piece.id, position))
        .collect();
    for relationship in relationships {
        for end in [relationship.from, relationship.to] {
            if let Some(&position) = positions.get(&end) {
                pieces[position].relationships.push(relationship);
            }
        }
    }
}

fn check_version(path: &Path, found: u32) -> Result<(), ReadChapterError> {
    if found == FORMAT_VERSION {
        return Ok(());
    }
    Err(ReadChapterError::FormatVersion {
        path: path.to_path_buf(),
        found,
        expected: FORMAT_VERSION,
    })
}

fn read_piece_file(file: &Path) -> Result<String, ReadChapterError> {
    let text = std::fs::read_to_string(file).map_err(|source| ReadChapterError::PieceFile {
        path: file.to_path_buf(),
        source,
    })?;
    Ok(text.strip_suffix('\n').unwrap_or(&text).to_owned())
}

/// The headings `entry` sits under. A heading is placed under the headings above it of a lower
/// rank number, and then becomes the innermost heading itself.
fn place_in_sections(
    open_sections: &mut Vec<SectionHeading>,
    entry: &PieceEntry,
    content: &str,
) -> Vec<SectionHeading> {
    let PieceDetail::Heading {
        rank,
        printed_number,
    } = &entry.detail
    else {
        return open_sections.clone();
    };
    while open_sections.last().is_some_and(|open| open.rank >= *rank) {
        open_sections.pop();
    }
    let section = open_sections.clone();
    open_sections.push(SectionHeading {
        rank: *rank,
        printed_number: printed_number.clone(),
        text: content.to_owned(),
    });
    section
}

fn mark_mid_sentence_pieces(page_pieces: &mut [ChapterPiece], page: &PageIndex) {
    let is_text = |piece: &&mut ChapterPiece| matches!(piece.detail, PieceDetail::Text { .. });
    if page.starts_mid_sentence
        && let Some(first) = page_pieces.iter_mut().find(is_text)
    {
        first.starts_mid_sentence = true;
    }
    if page.ends_mid_sentence
        && let Some(last) = page_pieces.iter_mut().rfind(is_text)
    {
        last.ends_mid_sentence = true;
    }
}

/// The saved relationships of every page, plus the `introduces` links that cross a page break.
fn collect_relationships(
    pages: &[(PathBuf, PageIndex)],
) -> Result<Vec<PieceRelationship>, ReadChapterError> {
    let mut relationships = Vec::new();
    for (position, (page_folder, page)) in (1..).zip(pages) {
        let has_number = |number: u32| page.pieces.iter().any(|piece| piece.number == number);
        for saved in &page.relationships {
            for number in [saved.from, saved.to] {
                if !has_number(number) {
                    return Err(ReadChapterError::UnknownPiece {
                        page_folder: page_folder.clone(),
                        number,
                    });
                }
            }
            relationships.push(PieceRelationship {
                kind: saved.kind,
                from: PieceId {
                    page: position,
                    number: saved.from,
                },
                to: PieceId {
                    page: position,
                    number: saved.to,
                },
            });
        }
    }
    for (previous_position, pair) in (1..).zip(pages.windows(2)) {
        relationships.extend(bridged_lead_ins(previous_position, &pair[0].1, &pair[1].1));
    }
    relationships.sort_by_key(|link| (link.from, link.to));
    Ok(relationships)
}

/// The lead-in of a formula that opens `next` is the last text piece of `previous`, when
/// `previous` ends in the middle of a sentence. Only figures, tables and footnotes may follow
/// that text piece, and only formulas may come before the formula.
fn bridged_lead_ins(
    previous_position: u32,
    previous: &PageIndex,
    next: &PageIndex,
) -> Vec<PieceRelationship> {
    if !previous.ends_mid_sentence {
        return Vec::new();
    }
    let lead_in = previous.pieces.iter().rev().find(|piece| {
        !matches!(
            piece.detail,
            PieceDetail::Figure { .. } | PieceDetail::Table { .. } | PieceDetail::Footnote { .. }
        )
    });
    let Some(lead_in) = lead_in.filter(|piece| matches!(piece.detail, PieceDetail::Text { .. }))
    else {
        return Vec::new();
    };
    next.pieces
        .iter()
        .take_while(|piece| matches!(piece.detail, PieceDetail::Formula { .. }))
        .filter(|formula| {
            !next.relationships.iter().any(|saved| {
                saved.kind == RelationshipKind::Introduces && saved.to == formula.number
            })
        })
        .map(|formula| PieceRelationship {
            kind: RelationshipKind::Introduces,
            from: PieceId {
                page: previous_position,
                number: lead_in.number,
            },
            to: PieceId {
                page: previous_position + 1,
                number: formula.number,
            },
        })
        .collect()
}
