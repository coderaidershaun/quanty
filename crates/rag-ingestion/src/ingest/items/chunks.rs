//! The text and footnote pieces, packed into chunks that stay inside one section and under a
//! size limit.

use ocr::{Chapter, ChapterPiece, PieceDetail, PieceId, RelationshipKind};
use rag_core::{DocumentInput, ItemKind};

use super::{BLOCK_SEPARATOR, Draft, context_line};

/// A chunk is closed before a paragraph that would take it past this.
const MAX_CHUNK_TOKENS: usize = 500;

/// A rough count, four characters to a token. It only shapes chunks, and the embedding model
/// accepts far more than a chunk holds, so a better count is not worth a tokenizer.
fn estimated_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(4)
}

/// One paragraph of the chapter as a reader sees it: one text piece, or several that a page
/// break cut apart.
struct Paragraph {
    pieces: Vec<PieceId>,
    printed_page: Option<String>,
    context: String,
    text: String,
    ends_mid_sentence: bool,
    footnotes: Vec<String>,
}

impl Paragraph {
    fn starting_with(piece: &ChapterPiece, doc_title: &str) -> Self {
        Self {
            pieces: vec![piece.id],
            printed_page: piece.printed_page_number.clone(),
            context: context_line(doc_title, &piece.section),
            text: piece.content.clone(),
            ends_mid_sentence: piece.ends_mid_sentence,
            footnotes: Vec::new(),
        }
    }

    /// A page break cut the sentence, so the next piece carries on from where this one stopped.
    fn continue_with(&mut self, piece: &ChapterPiece) {
        self.pieces.push(piece.id);
        // SMELL: a word that the page break cut in two with a hyphen stays in two parts, with
        // this space between them.
        self.text.push(' ');
        self.text.push_str(&piece.content);
        self.ends_mid_sentence = piece.ends_mid_sentence;
    }

    fn text_with_footnotes(&self) -> String {
        std::iter::once(self.text.as_str())
            .chain(self.footnotes.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(BLOCK_SEPARATOR)
    }
}

struct Chunk {
    first_piece: PieceId,
    printed_page: Option<String>,
    context: String,
    text: String,
}

impl Chunk {
    fn starting_with(paragraph: &Paragraph, text: String) -> Self {
        Self {
            first_piece: paragraph.pieces[0],
            // SMELL: a chunk that runs over a page break keeps only the page it starts on, so
            // its text from the next page is cited with the number of the page before.
            printed_page: paragraph.printed_page.clone(),
            context: paragraph.context.clone(),
            text,
        }
    }

    fn of_footnote(footnote: &ChapterPiece, doc_title: &str) -> Self {
        Self {
            first_piece: footnote.id,
            printed_page: footnote.printed_page_number.clone(),
            context: context_line(doc_title, &footnote.section),
            text: footnote.content.clone(),
        }
    }

    fn into_draft(self) -> Draft {
        Draft {
            first_piece: self.first_piece,
            printed_page: self.printed_page,
            kind: ItemKind::Chunk,
            input: DocumentInput {
                title: self.context,
                text: self.text.clone(),
                image: None,
            },
            text: self.text,
        }
    }
}

pub(super) fn chunk_drafts(chapter: &Chapter, doc_title: &str) -> Vec<Draft> {
    let mut runs = paragraph_runs(chapter, doc_title);
    let lone_footnotes = attach_footnotes(chapter, &mut runs);

    let packed = runs.into_iter().flat_map(pack_into_chunks);
    let alone = lone_footnotes
        .into_iter()
        .map(|footnote| Chunk::of_footnote(footnote, doc_title));
    packed.chain(alone).map(Chunk::into_draft).collect()
}

/// The paragraphs of the chapter in groups that a heading separates. A chunk is made from one
/// group only, so it never crosses a heading.
fn paragraph_runs(chapter: &Chapter, doc_title: &str) -> Vec<Vec<Paragraph>> {
    let mut runs = Vec::new();
    let mut current: Vec<Paragraph> = Vec::new();
    for piece in &chapter.pieces {
        match &piece.detail {
            PieceDetail::Heading { .. } => runs.push(std::mem::take(&mut current)),
            PieceDetail::Text { .. } if !piece.content.trim().is_empty() => {
                // Both flags are needed. A page that ends mid-sentence before a formula has no
                // second half to join, and a page that starts mid-sentence after a finished
                // sentence has no first half.
                let cut_paragraph = current
                    .last_mut()
                    .filter(|earlier| earlier.ends_mid_sentence && piece.starts_mid_sentence);
                match cut_paragraph {
                    Some(earlier) => earlier.continue_with(piece),
                    None => current.push(Paragraph::starting_with(piece, doc_title)),
                }
            }
            _ => {}
        }
    }
    runs.push(current);
    runs.retain(|run| !run.is_empty());
    runs
}

/// Adds each footnote to the paragraph it belongs to and returns the footnotes that belong to
/// none.
///
/// The converter links a footnote to every piece that shows its marker, so it is added once
/// only, to the first of those in reading order that is a text piece. This is a pass of its own
/// because a footnote sits at the foot of its page, where the section of its paragraph may be
/// over.
fn attach_footnotes<'a>(
    chapter: &'a Chapter,
    runs: &mut [Vec<Paragraph>],
) -> Vec<&'a ChapterPiece> {
    let mut lone = Vec::new();
    for footnote in &chapter.pieces {
        let PieceDetail::Footnote { marker, .. } = &footnote.detail else {
            continue;
        };
        let mut targets: Vec<PieceId> = footnote
            .relationships
            .iter()
            .filter(|link| link.kind == RelationshipKind::FootnoteOf && link.from == footnote.id)
            .map(|link| link.to)
            .collect();
        targets.sort();
        match targets.iter().find_map(|id| locate(runs, *id)) {
            Some((run, paragraph)) => {
                let block = match marker {
                    Some(marker) => format!("[^{marker}]: {}", footnote.content),
                    None => format!("Footnote: {}", footnote.content),
                };
                runs[run][paragraph].footnotes.push(block);
            }
            None => lone.push(footnote),
        }
    }
    lone
}

/// The run and the place in it of the paragraph that holds the text piece.
fn locate(runs: &[Vec<Paragraph>], id: PieceId) -> Option<(usize, usize)> {
    runs.iter().enumerate().find_map(|(run, paragraphs)| {
        let paragraph = paragraphs
            .iter()
            .position(|paragraph| paragraph.pieces.contains(&id))?;
        Some((run, paragraph))
    })
}

/// Adds paragraphs to the open chunk until the next one would take it past the limit. A
/// paragraph is never split, so one that is over the limit on its own is a chunk by itself.
fn pack_into_chunks(run: Vec<Paragraph>) -> Vec<Chunk> {
    let mut chunks: Vec<Chunk> = Vec::new();
    for paragraph in run {
        let block = paragraph.text_with_footnotes();
        let open = chunks.last_mut().filter(|chunk| {
            let joined = format!("{}{BLOCK_SEPARATOR}{block}", chunk.text);
            estimated_tokens(&joined) <= MAX_CHUNK_TOKENS
        });
        match open {
            Some(chunk) => {
                chunk.text.push_str(BLOCK_SEPARATOR);
                chunk.text.push_str(&block);
            }
            None => chunks.push(Chunk::starting_with(&paragraph, block)),
        }
    }
    chunks
}
