//! Turns the pieces of a converted chapter into blocks of Markdown in reading order, each with the
//! headings it sits under, the notes it gives the model writers, and the picture it shows.

use std::collections::HashMap;
use std::path::PathBuf;

use ocr::{Chapter, ChapterPiece, ImageShows, PieceDetail, PieceId, RelationshipKind};

use super::Provenance;
use super::markdown::{
    explanation_of, fenced, figure_name, formula_note, formula_text, heading_text, inline_math,
    non_blank, with_label_line,
};
use super::tokens::estimated_tokens;

/// One unit of rendered text, with the pieces it holds.
pub(super) struct Block {
    pub text: String,
    /// The headings it sits under, as numbers in the order the headings come in the chapter,
    /// outermost first. A heading's own block carries the heading in its path.
    pub path: Vec<usize>,
    /// In the order they are written.
    pub pieces: Vec<PieceId>,
    pub tokens: usize,
    pub provenance: Provenance,
    /// What a model that writes from this section is told besides the text.
    pub note: Option<String>,
    /// The picture of a figure, when it shows the figure and not the whole page.
    pub image: Option<PathBuf>,
    /// The block after this one belongs with it, so a cut must not fall between them.
    pub keeps_with_next: bool,
    /// A footnote that no text carries. It is written at the end of its section.
    pub is_lone_footnote: bool,
}

pub(super) struct Rendered {
    pub blocks: Vec<Block>,
    /// The text of every heading as saved, with its printed number, by the number the paths use.
    pub headings: Vec<String>,
}

/// The earlier half of a paragraph that a page break cut, the space, and the later half.
// SMELL: a word that the page break cut in two with a hyphen stays in two parts, with the hyphen
// and this space between them.
fn join_cut_text(earlier: &str, later: &str) -> String {
    format!("{earlier} {later}")
}

/// A block of the chapter before the footnotes are placed.
enum Slot {
    Block(Block),
    Footnote {
        block: Block,
        /// The pieces that show the footnote's marker, in reading order.
        carriers: Vec<PieceId>,
    },
}

pub(super) fn render(chapter: &Chapter) -> Rendered {
    let mut state = Rendering::default();
    for piece in &chapter.pieces {
        state.add(piece);
    }
    let mut blocks = place_footnotes(state.slots, &state.paragraph_of);
    mark_lead_ins(chapter, &mut blocks);
    for block in &mut blocks {
        block.tokens = estimated_tokens(&block.text);
    }
    Rendered {
        blocks,
        headings: state.headings,
    }
}

#[derive(Default)]
struct Rendering {
    slots: Vec<Slot>,
    headings: Vec<String>,
    /// The headings the next piece sits under.
    open_path: Vec<usize>,
    /// The paragraph that a text piece is part of, by the place of its block.
    paragraph_of: HashMap<PieceId, usize>,
    /// The latest paragraph since the last heading, and whether it stops in the middle of a
    /// sentence.
    latest_paragraph: Option<(usize, bool)>,
}

impl Rendering {
    fn add(&mut self, piece: &ChapterPiece) {
        let content = piece.content.as_str();
        match &piece.detail {
            PieceDetail::Heading {
                rank,
                printed_number,
            } => self.add_heading(piece, *rank, printed_number.as_deref()),
            PieceDetail::Text { .. } => self.add_text(piece),
            PieceDetail::Formula {
                label,
                name,
                statement,
                symbols,
            } => {
                let text = formula_text(content, label.as_deref());
                let note = formula_note(label.as_deref(), name.as_deref(), statement, symbols);
                self.push(piece, text, estimated_tokens(content), note);
            }
            PieceDetail::Table { label, caption, .. } => {
                let text =
                    with_label_line(label.as_deref(), caption.as_deref(), &inline_math(content));
                self.push(piece, text, estimated_tokens(content), None);
            }
            PieceDetail::Code {
                label,
                caption,
                language,
                ..
            } => {
                let listing = fenced(content, language.as_deref());
                let text = with_label_line(label.as_deref(), caption.as_deref(), &listing);
                self.push(piece, text, estimated_tokens(content), None);
            }
            PieceDetail::Figure {
                label,
                caption,
                printed_text,
                ..
            } => self.add_figure(piece, label.as_deref(), caption.as_deref(), printed_text),
            PieceDetail::Footnote { marker, .. } => self.add_footnote(piece, marker.as_deref()),
        }
    }

    fn add_heading(&mut self, piece: &ChapterPiece, rank: u8, printed_number: Option<&str>) {
        let saved = heading_text(printed_number, &piece.content);
        let number = self.headings.len();
        self.headings.push(saved.clone());
        // The headings above this one are the ones in the piece's section.
        self.open_path.truncate(piece.section.len());
        self.open_path.push(number);
        self.latest_paragraph = None;
        let text = format!(
            "{} {}",
            "#".repeat(usize::from(rank.clamp(1, 6))),
            inline_math(&saved)
        );
        self.push(piece, text, estimated_tokens(&piece.content), None);
    }

    fn add_text(&mut self, piece: &ChapterPiece) {
        if piece.content.trim().is_empty() {
            return;
        }
        let text = inline_math(&piece.content);
        let tokens = estimated_tokens(&piece.content);
        let cut_paragraph = self
            .latest_paragraph
            .filter(|(_, stops_mid_sentence)| *stops_mid_sentence && piece.starts_mid_sentence);
        match cut_paragraph {
            Some((slot, _)) => {
                if let Slot::Block(block) = &mut self.slots[slot] {
                    block.text = join_cut_text(&block.text, &text);
                    block.pieces.push(piece.id);
                    block.provenance.transcribed_tokens += tokens;
                }
                self.paragraph_of.insert(piece.id, slot);
                self.latest_paragraph = Some((slot, piece.ends_mid_sentence));
            }
            None => {
                let slot = self.slots.len();
                self.paragraph_of.insert(piece.id, slot);
                self.latest_paragraph = Some((slot, piece.ends_mid_sentence));
                self.push(piece, text, tokens, None);
            }
        }
    }

    fn add_figure(
        &mut self,
        piece: &ChapterPiece,
        label: Option<&str>,
        caption: Option<&str>,
        printed_text: &[String],
    ) {
        let explanation = explanation_of(&piece.content);
        let mut head = figure_name(label);
        if let Some(caption) = non_blank(caption) {
            head = format!("{head}: {}", inline_math(caption));
        }
        let text = format!(
            "[{head} — description (model-written): {}]",
            inline_math(explanation)
        );
        let note = (!printed_text.is_empty()).then(|| {
            let quoted: Vec<String> = printed_text
                .iter()
                .map(|text| format!("\"{text}\""))
                .collect();
            format!("{} printed text: {}", figure_name(label), quoted.join("; "))
        });
        let mut block = self.block_of(piece, text, 0, note);
        block.provenance.model_written_tokens = estimated_tokens(explanation);
        block.image = piece
            .figure_image
            .as_ref()
            .filter(|picture| matches!(picture.shows, ImageShows::Figure { .. }))
            .map(|picture| picture.path.clone());
        self.slots.push(Slot::Block(block));
    }

    fn add_footnote(&mut self, piece: &ChapterPiece, marker: Option<&str>) {
        let footnote = inline_math(&piece.content);
        let text = match marker {
            Some(marker) => format!("[^{marker}]: {footnote}"),
            None => format!("Footnote: {footnote}"),
        };
        let mut block = self.block_of(piece, text, estimated_tokens(&piece.content), None);
        // Every footnote starts out lone. Placing the footnotes takes the mark off a footnote that
        // a text carries, and moves it after that text.
        block.is_lone_footnote = true;
        let carriers = carriers_of(piece);
        self.slots.push(Slot::Footnote { block, carriers });
    }

    fn push(
        &mut self,
        piece: &ChapterPiece,
        text: String,
        transcribed_tokens: usize,
        note: Option<String>,
    ) {
        let block = self.block_of(piece, text, transcribed_tokens, note);
        self.slots.push(Slot::Block(block));
    }

    fn block_of(
        &self,
        piece: &ChapterPiece,
        text: String,
        transcribed_tokens: usize,
        note: Option<String>,
    ) -> Block {
        Block {
            text,
            path: self.open_path.clone(),
            pieces: vec![piece.id],
            tokens: 0,
            provenance: Provenance {
                transcribed_tokens,
                model_written_tokens: 0,
            },
            note,
            image: None,
            keeps_with_next: false,
            is_lone_footnote: false,
        }
    }
}

/// A footnote follows the first text that shows its marker, and takes that text's headings. One
/// that no text carries stays where the page printed it.
fn place_footnotes(slots: Vec<Slot>, paragraph_of: &HashMap<PieceId, usize>) -> Vec<Block> {
    let carrier_slots: Vec<Option<usize>> = slots
        .iter()
        .map(|slot| match slot {
            Slot::Footnote { carriers, .. } => carriers
                .iter()
                .find_map(|carrier| paragraph_of.get(carrier).copied()),
            Slot::Block(_) => None,
        })
        .collect();
    let paths: Vec<Vec<usize>> = slots
        .iter()
        .map(|slot| match slot {
            Slot::Block(block) | Slot::Footnote { block, .. } => block.path.clone(),
        })
        .collect();
    let mut after: HashMap<usize, Vec<Block>> = HashMap::new();
    let mut in_place = Vec::new();
    for (position, slot) in slots.into_iter().enumerate() {
        match (slot, carrier_slots[position]) {
            (Slot::Footnote { mut block, .. }, Some(carrier)) => {
                block.path = paths[carrier].clone();
                block.is_lone_footnote = false;
                after.entry(carrier).or_default().push(block);
            }
            (Slot::Footnote { block, .. } | Slot::Block(block), _) => {
                in_place.push((position, block));
            }
        }
    }
    let mut blocks = Vec::new();
    for (position, block) in in_place {
        blocks.push(block);
        blocks.extend(after.remove(&position).unwrap_or_default());
    }
    blocks
}

/// The text pieces that a footnote is a footnote of, earliest first.
fn carriers_of(footnote: &ChapterPiece) -> Vec<PieceId> {
    let mut carriers: Vec<PieceId> = footnote
        .relationships
        .iter()
        .filter(|link| link.kind == RelationshipKind::FootnoteOf && link.from == footnote.id)
        .map(|link| link.to)
        .collect();
    carriers.sort();
    carriers
}

/// Marks each block from a text that leads into a formula up to the formula, so that a cut never
/// separates them.
fn mark_lead_ins(chapter: &Chapter, blocks: &mut [Block]) {
    let block_of: HashMap<PieceId, usize> = blocks
        .iter()
        .enumerate()
        .flat_map(|(position, block)| block.pieces.iter().map(move |id| (*id, position)))
        .collect();
    for formula in &chapter.pieces {
        for link in &formula.relationships {
            if link.kind != RelationshipKind::Introduces || link.to != formula.id {
                continue;
            }
            let (Some(&lead_in), Some(&formula_block)) =
                (block_of.get(&link.from), block_of.get(&formula.id))
            else {
                continue;
            };
            for block in blocks.iter_mut().take(formula_block).skip(lead_in) {
                block.keeps_with_next = true;
            }
        }
    }
}
