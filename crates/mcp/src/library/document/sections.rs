//! The `read_section` and `read_pages` tools: a section of a converted chapter, or a run of its
//! pages, as Markdown or as pieces, within a budget of characters.

use std::collections::HashMap;
use std::fmt;

use ocr::{Chapter, ChapterPiece, PieceId, read_chapter};
use rag_core::{Config, DocId, ItemId, error_chain, is_same_name};
use rag_dataset::section::reading_blocks;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::args::{Budget, Scope, Truncation};
use crate::library::LibraryError;
use crate::library::chapter::{DocumentHeader, Whole, find_chapter, parse_document_id};
use crate::library::piece::PieceView;

/// What a section's headings are joined with to name it.
pub(super) const PATH_SEPARATOR: &str = " › ";

/// How a read gives its text.
#[derive(Deserialize, JsonSchema, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Form {
    /// One text of Markdown.
    #[default]
    Markdown,
    /// The pieces, each with its kind, label and caption.
    Pieces,
}

impl fmt::Display for Form {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Form::Markdown => "markdown",
            Form::Pieces => "pieces",
        })
    }
}

/// A section by its place in the outline, or by its heading.
#[derive(Deserialize, JsonSchema)]
#[serde(untagged)]
pub(crate) enum SectionChoice {
    Index(usize),
    Heading(String),
}

/// What an agent gives `read_section`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct ReadSectionArgs {
    /// From `list_documents` or a search result.
    pub(crate) document_id: String,
    /// The `index` of a section of `document_outline`, or its heading, whatever the capitals.
    pub(crate) section: SectionChoice,
    /// `markdown` (default) or `pieces`.
    #[serde(rename = "as")]
    pub(crate) form: Option<Form>,
    /// At most this many characters of text. 24000 when left out.
    pub(crate) max_chars: Option<u32>,
    /// From a result that was `truncated`, with the same other arguments.
    pub(crate) cursor: Option<String>,
}

/// One section of a document.
#[derive(Serialize, JsonSchema)]
pub(crate) struct SectionView {
    #[serde(flatten)]
    document: DocumentHeader,
    index: usize,
    path: Vec<String>,
    /// The first and the last page of the section, counted from 1.
    pages: [u32; 2],
    /// The stored items of the section. Give one to `read_item`.
    item_ids: Vec<String>,
    /// The picture URLs of its figures, relative to the app base.
    pictures: Vec<String>,
    /// The Markdown of the section. With `as: markdown`.
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    /// With `as: pieces`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pieces: Option<Vec<PieceView>>,
    #[serde(flatten)]
    more: Truncation,
}

impl SectionView {
    pub(crate) fn truncation(&self) -> &Truncation {
        &self.more
    }
}

/// One section of the chapter that the document id names, as Markdown or as pieces. It needs no
/// store.
///
/// # Errors
/// A bad argument, a section that the document does not have, a document that no converted
/// chapter has, or a chapter that cannot be read.
pub(crate) fn read_section(
    config: &Config,
    args: ReadSectionArgs,
) -> Result<SectionView, LibraryError> {
    let id = parse_document_id(&args.document_id)?;
    let whole = Whole::find(&config.content_folder, id)?;
    let index = section_index(&whole, &args.section)?;
    let (section, _) = &whole.sections[index - 1];
    let form = args.form.unwrap_or_default();
    let scope = Scope::of("read_section").with(id).with(index).with(form);
    let budget = Budget::new(scope, args.max_chars, args.cursor.as_deref())?;
    let (text, pieces, more) = match form {
        Form::Markdown => {
            let (text, more) = budget.take_text(&section.markdown)?;
            (Some(text), None, more)
        }
        Form::Pieces => {
            let by_id: HashMap<PieceId, &ChapterPiece> = whole
                .chapter
                .pieces
                .iter()
                .map(|piece| (piece.id, piece))
                .collect();
            let of_section: Vec<&ChapterPiece> = section
                .piece_ids
                .iter()
                .filter_map(|id| by_id.get(id).copied())
                .collect();
            let (views, more) = PieceView::within(&of_section, &budget, &config.content_folder)?;
            (None, Some(views), more)
        }
    };
    Ok(SectionView {
        document: DocumentHeader::of(&whole.entry),
        index,
        path: section.section_path.clone(),
        pages: page_range(&section.pages),
        item_ids: section.item_ids.iter().map(ToString::to_string).collect(),
        pictures: section
            .images
            .iter()
            .filter_map(|image| rag_core::content_url(&image.path, &config.content_folder))
            .collect(),
        text,
        pieces,
        more,
    })
}

/// The place of the section the caller chose, counted from 1.
fn section_index(whole: &Whole, choice: &SectionChoice) -> Result<usize, LibraryError> {
    let sections = whole.sections.len();
    let name = match choice {
        SectionChoice::Index(index) if (1..=sections).contains(index) => return Ok(*index),
        SectionChoice::Index(index) => {
            return Err(LibraryError::SectionOutOfRange {
                index: *index,
                sections,
            });
        }
        SectionChoice::Heading(name) => name,
    };
    let matching: Vec<usize> = whole
        .sections
        .iter()
        .enumerate()
        .filter(|(_, (section, _))| {
            is_same_name(&section.section_path.join(PATH_SEPARATOR), name)
                || section
                    .section_path
                    .last()
                    .is_some_and(|last| is_same_name(last, name))
        })
        .map(|(place, _)| place + 1)
        .collect();
    match matching.as_slice() {
        [] => Err(LibraryError::NoSuchSection { name: name.clone() }),
        [only] => Ok(*only),
        several => Err(LibraryError::AmbiguousSection {
            name: name.clone(),
            indexes: several
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", "),
        }),
    }
}

/// The first and the last of the pages, or 0 for a section with none.
pub(super) fn page_range(pages: &[u32]) -> [u32; 2] {
    [
        pages.first().copied().unwrap_or(0),
        pages.last().copied().unwrap_or(0),
    ]
}

/// What an agent gives `read_pages`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct ReadPagesArgs {
    /// From `list_documents` or a search result.
    pub(crate) document_id: String,
    /// The first page, counted from 1.
    pub(crate) from: u32,
    /// The last page, no earlier than `from`.
    pub(crate) to: u32,
    /// `markdown` (default) or `pieces`.
    #[serde(rename = "as")]
    pub(crate) form: Option<Form>,
    /// At most this many characters of text. 24000 when left out.
    pub(crate) max_chars: Option<u32>,
    /// From a result that was `truncated`, with the same other arguments.
    pub(crate) cursor: Option<String>,
}

/// A run of pages of one document.
#[derive(Serialize, JsonSchema)]
pub(crate) struct PagesView {
    #[serde(flatten)]
    document: DocumentHeader,
    from: u32,
    to: u32,
    /// The Markdown of the pages. With `as: markdown`.
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    /// With `as: pieces`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pieces: Option<Vec<PieceView>>,
    #[serde(flatten)]
    more: Truncation,
}

impl PagesView {
    pub(crate) fn truncation(&self) -> &Truncation {
        &self.more
    }
}

/// The pages `from` to `to` of the chapter that the document id names. As Markdown, a paragraph
/// that starts on a page of the run is given whole, even when it ends on the page after `to`. It
/// needs no store.
///
/// # Errors
/// A bad argument, pages that the chapter does not have, a document that no converted chapter has,
/// or a chapter that cannot be read.
pub(crate) fn read_pages(config: &Config, args: ReadPagesArgs) -> Result<PagesView, LibraryError> {
    let id = parse_document_id(&args.document_id)?;
    let (from, to) = (args.from, args.to);
    if from == 0 || to == 0 {
        return Err(LibraryError::ZeroPage);
    }
    if from > to {
        return Err(LibraryError::PagesReversed { from, to });
    }
    let entry = find_chapter(&config.content_folder, id)?;
    if to > entry.index.page_count {
        return Err(LibraryError::PageOutOfRange {
            page: to,
            pages: entry.index.page_count,
        });
    }
    let chapter = read_chapter(&entry.folder)?;
    let form = args.form.unwrap_or_default();
    let scope = Scope::of("read_pages")
        .with(id)
        .with(from)
        .with(to)
        .with(form);
    let budget = Budget::new(scope, args.max_chars, args.cursor.as_deref())?;
    let (text, pieces, more) = match form {
        Form::Markdown => {
            let (text, more) = budget.take_text(&markdown_of_pages(&chapter, from, to))?;
            (Some(text), None, more)
        }
        Form::Pieces => {
            let on_pages: Vec<&ChapterPiece> = chapter
                .pieces
                .iter()
                .filter(|piece| (from..=to).contains(&piece.id.page))
                .collect();
            let (views, more) = PieceView::within(&on_pages, &budget, &config.content_folder)?;
            (None, Some(views), more)
        }
    };
    Ok(PagesView {
        document: DocumentHeader::of(&entry),
        from,
        to,
        text,
        pieces,
        more,
    })
}

/// The blocks of the chapter whose first piece is on one of the pages, as one Markdown text.
fn markdown_of_pages(chapter: &Chapter, from: u32, to: u32) -> String {
    reading_blocks(chapter)
        .into_iter()
        .filter(|block| {
            block
                .pieces
                .first()
                .is_some_and(|piece| (from..=to).contains(&piece.page))
        })
        .map(|block| block.markdown)
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Where an item sits in the outline of its document.
#[derive(Serialize, JsonSchema, Clone)]
pub(crate) struct SectionRef {
    /// The `index` of the section in `document_outline`, counted from 1.
    index: usize,
    /// The headings above the section, outermost first.
    path: Vec<String>,
}

/// The section that holds each item of the document, by item. An item that is in two sections is
/// given the first. It is empty when the document has no converted chapter that can be read, so
/// that a search still answers; a chapter that is there and cannot be read is logged.
pub(crate) fn sections_of_items(config: &Config, id: DocId) -> HashMap<ItemId, SectionRef> {
    let whole = match Whole::find(&config.content_folder, id) {
        Ok(whole) => whole,
        Err(LibraryError::UnknownDocument { .. }) => return HashMap::new(),
        Err(error) => {
            tracing::warn!(
                document = %id,
                error = %error_chain(&error),
                "the results of this document show no section: its chapter cannot be read"
            );
            return HashMap::new();
        }
    };
    let mut places = HashMap::new();
    for (place, (section, _)) in whole.sections.iter().enumerate() {
        for item in &section.item_ids {
            places.entry(*item).or_insert_with(|| SectionRef {
                index: place + 1,
                path: section.section_path.clone(),
            });
        }
    }
    places
}

/// The stored items of the section with this `index`, for a search that looks inside it.
///
/// # Errors
/// A document that no converted chapter has, a chapter that cannot be read, or a section that the
/// document does not have.
pub(crate) fn items_of_section(
    config: &Config,
    id: DocId,
    index: usize,
) -> Result<Vec<ItemId>, LibraryError> {
    let whole = Whole::find(&config.content_folder, id)?;
    let section = section_index(&whole, &SectionChoice::Index(index))?;
    Ok(whole.sections[section - 1].0.item_ids.clone())
}
