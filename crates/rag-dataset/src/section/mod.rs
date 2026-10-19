//! Cuts a converted chapter into whole sections of Markdown, with the pages, pieces and stored
//! items each section holds, so that a model trains on a book the way a reader meets it.

mod cut;
mod markdown;
mod render;
mod tokens;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;

use ocr::{Chapter, ChapterPiece, DocumentName, PieceDetail, PieceId, SectionHeading};
use rag_core::{ItemId, is_same_name};
use rag_ingestion::Item;
use serde::{Deserialize, Serialize};

use crate::documents::ExportDocument;

use cut::Unit;
use markdown::{heading_text, inline_math};
use render::{Block, Rendered};

pub use tokens::estimated_tokens;

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
    let Rendered { blocks, headings } = render::render(chapter);
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
            sections.push(maker.section(id, unit, positions));
        }
    }
    sections
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
    }

    /// The items whose title is the context line of a piece of the section: the stored items
    /// that were made from the same part of the chapter.
    fn item_ids_of(&self, piece_ids: &[PieceId]) -> Vec<ItemId> {
        let context_lines: HashSet<String> = piece_ids
            .iter()
            .filter_map(|id| self.pieces.get(id))
            .filter(|piece| !matches!(piece.detail, PieceDetail::Heading { .. }))
            .map(|piece| context_line(&self.document.title, &piece.section))
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

/// The document title and then the headings a piece sits under, outermost first. Ingestion titles
/// the items of a chapter the same way, and a section finds its items by that title.
// SMELL: a copy of the rule that ingestion titles its items by, which ingestion keeps private. The
// two must stay the same, or the sections lose their items.
fn context_line(document_title: &str, section: &[SectionHeading]) -> String {
    std::iter::once(document_title.to_owned())
        .chain(
            section
                .iter()
                .map(|heading| heading_text(heading.printed_number.as_deref(), &heading.text)),
        )
        .collect::<Vec<_>>()
        .join(" > ")
}
