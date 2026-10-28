//! Cuts a converted chapter into whole sections of Markdown, and each section into the passages a
//! writer reads, so that a model trains on a book the way a reader meets it.

mod cut;
mod markdown;
mod passage;
mod render;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;

use ocr::{Chapter, ChapterPiece, DocumentName, PieceDetail, PieceId};
use rag_core::{ItemId, is_same_name};
use rag_ingestion::Item;
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use crate::documents::ExportDocument;

use cut::Unit;
use markdown::inline_math;
use render::{Block, Rendered};

pub(crate) use passage::{body_of, passages_of};
pub use rag_core::estimated_tokens;
pub use render::HeadingMark;

/// A picture that a section's text points at: the file, and where in the text its note starts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageRef {
    #[serde(rename = "ref")]
    pub path: PathBuf,
    /// The offset in characters, not bytes, of the `[` that starts the figure's note.
    pub pos: usize,
}

/// How much of a text the book printed and how much a model wrote for it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub transcribed_tokens: usize,
    /// The explanations of figures, which a model wrote from the picture.
    pub model_written_tokens: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// `<media folder>:<document folder>:s<index>`, with `:p<part>` for a part of a long section.
    pub id: String,
    pub document: ExportDocument,
    /// The headings the section sits under as saved, outermost first. Empty for a whole chapter.
    pub section_path: Vec<String>,
    /// The positions of the pages in the chapter's PDF, not the numbers printed on them.
    pub pages: Vec<u32>,
    pub piece_ids: Vec<PieceId>,
    pub item_ids: Vec<ItemId>,
    pub markdown: String,
    /// What a model that writes from this section is told besides its text.
    pub notes: String,
    pub images: Vec<ImageRef>,
    pub n_tokens: usize,
    pub provenance: Provenance,
}

const BLOCK_SEPARATOR: &str = "\n\n";
const HEADING_SEPARATOR: &str = " › ";

impl Section {
    /// The text after the heading line and the blank line that follows it, or the whole text when
    /// there is no blank line. The heading line names the media and the document, so two copies of
    /// one text under two titles differ only there.
    pub(crate) fn text_after_heading_line(&self) -> &str {
        self.markdown
            .split_once(BLOCK_SEPARATOR)
            .map_or(self.markdown.as_str(), |(_, rest)| rest)
    }
}

/// `<media folder>:<document folder>:s<index>` with a `:p<part>` after it when a long section was
/// cut in parts. The index and the part start at 1.
pub fn section_id(document: &ExportDocument, index: usize, part: Option<usize>) -> String {
    let slug = document.slug();
    match part {
        Some(part) => format!("{slug}:s{index:03}:p{part}"),
        None => format!("{slug}:s{index:03}"),
    }
}

/// The sections of a chapter in reading order, with ids that do not change while the chapter does
/// not. `items` are the items that ingestion makes of the same chapter.
pub fn sections_of(document: &ExportDocument, chapter: &Chapter, items: &[Item]) -> Vec<Section> {
    sections_with_headings(document, chapter, items)
        .into_iter()
        .map(|(section, _)| section)
        .collect()
}

/// The sections of [`sections_of`], each with the mark of the heading that names it: the last
/// heading of its path. A section that sits under no heading has none.
pub fn sections_with_headings(
    document: &ExportDocument,
    chapter: &Chapter,
    items: &[Item],
) -> Vec<(Section, Option<HeadingMark>)> {
    let Rendered {
        blocks,
        headings,
        marks,
    } = render::render(chapter);
    let pieces: HashMap<PieceId, &ChapterPiece> = chapter
        .pieces
        .iter()
        .map(|piece| (piece.id, piece))
        .collect();
    let maker = SectionMaker {
        document,
        chapter,
        items,
        blocks: &blocks,
        headings: &headings,
        pieces: &pieces,
    };
    let mut sections = Vec::new();
    for (index, unit) in cut::cut(&blocks).iter().enumerate() {
        let several_parts = unit.parts.len() > 1;
        for (part, positions) in unit.parts.iter().enumerate() {
            let id = section_id(document, index + 1, several_parts.then_some(part + 1));
            let mark = unit.path.last().map(|&number| marks[number].clone());
            sections.push((maker.section(id, unit, positions), mark));
        }
    }
    sections
}

/// One block of a chapter as a reader meets it: its Markdown, and the pieces it is made of. A
/// paragraph that a page break cut is one block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadingBlock {
    pub markdown: String,
    /// In the order they are written.
    pub pieces: Vec<PieceId>,
}

/// The blocks of a chapter in reading order, written as they are in a section. A footnote follows
/// the paragraph that shows its marker.
pub fn reading_blocks(chapter: &Chapter) -> Vec<ReadingBlock> {
    render::render(chapter)
        .blocks
        .into_iter()
        .map(|block| ReadingBlock {
            markdown: block.text,
            pieces: block.pieces,
        })
        .collect()
}

struct SectionMaker<'a> {
    document: &'a ExportDocument,
    chapter: &'a Chapter,
    items: &'a [Item],
    blocks: &'a [Block],
    headings: &'a [String],
    pieces: &'a HashMap<PieceId, &'a ChapterPiece>,
}

impl SectionMaker<'_> {
    fn section(&self, id: String, unit: &Unit, positions: &[usize]) -> Section {
        let blocks: Vec<&Block> = positions
            .iter()
            .map(|&position| &self.blocks[position])
            .collect();
        let section_path: Vec<String> = unit
            .path
            .iter()
            .map(|&number| self.headings[number].clone())
            .collect();
        let heading_line = self.heading_line(&section_path);
        let markdown = std::iter::once(heading_line.as_str())
            .chain(blocks.iter().map(|block| block.text.as_str()))
            .collect::<Vec<_>>()
            .join(BLOCK_SEPARATOR);
        let piece_ids: Vec<PieceId> = blocks
            .iter()
            .flat_map(|block| block.pieces.clone())
            .collect();
        let pages: BTreeSet<u32> = piece_ids.iter().map(|id| id.page).collect();
        let notes = blocks
            .iter()
            .filter_map(|block| block.note.as_deref())
            .collect::<Vec<_>>()
            .join(BLOCK_SEPARATOR);
        Section {
            id,
            document: self.document.clone(),
            section_path,
            pages: pages.into_iter().collect(),
            item_ids: self.item_ids_of(&piece_ids),
            piece_ids,
            n_tokens: estimated_tokens(&markdown),
            markdown,
            notes,
            images: images_of(&heading_line, &blocks),
            provenance: provenance_of(&blocks),
        }
    }

    /// `# <media> › <chapter or document> › <heading> › …`. A paper that is named after its media
    /// says the name once.
    fn heading_line(&self, section_path: &[String]) -> String {
        let media = &self.document.media;
        let own_name = match &self.chapter.index.name {
            DocumentName::Chapter { name, .. } => name.as_str(),
            DocumentName::Title(title) => ocr::content::own_title(media, title),
        };
        let parts = std::iter::once(media.as_str())
            .chain((!is_same_name(own_name, media)).then_some(own_name))
            .chain(section_path.iter().map(String::as_str))
            .map(inline_math)
            .collect::<Vec<_>>();
        format!("# {}", parts.join(HEADING_SEPARATOR))
            .nfc()
            .collect()
    }

    /// The items whose title is the context line of a piece of the section: the stored items
    /// that were made from the same part of the chapter.
    fn item_ids_of(&self, piece_ids: &[PieceId]) -> Vec<ItemId> {
        let context_lines: HashSet<String> = piece_ids
            .iter()
            .filter_map(|id| self.pieces.get(id))
            .filter(|piece| !matches!(piece.detail, PieceDetail::Heading { .. }))
            .map(|piece| rag_ingestion::context_line(&self.document.title, &piece.section))
            .collect();
        self.items
            .iter()
            .filter(|item| context_lines.contains(&item.input.title))
            .map(|item| item.id)
            .collect()
    }
}

/// Where the note of each picture starts in the section's text, in characters. The count follows
/// how the text is joined: the heading line, then each block, with a blank line between them.
fn images_of(heading_line: &str, blocks: &[&Block]) -> Vec<ImageRef> {
    let mut images = Vec::new();
    let mut offset = heading_line.chars().count() + BLOCK_SEPARATOR.chars().count();
    for block in blocks {
        if let Some(path) = &block.image {
            images.push(ImageRef {
                path: path.clone(),
                pos: offset,
            });
        }
        offset += block.text.chars().count() + BLOCK_SEPARATOR.chars().count();
    }
    images
}

fn provenance_of(blocks: &[&Block]) -> Provenance {
    Provenance {
        transcribed_tokens: blocks
            .iter()
            .map(|block| block.provenance.transcribed_tokens)
            .sum(),
        model_written_tokens: blocks
            .iter()
            .map(|block| block.provenance.model_written_tokens)
            .sum(),
    }
}
