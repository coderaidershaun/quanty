//! The two model replies for a page, the schemas that force their shape, and the two calls that
//! ask for them: a plain copy by Haiku, or a full transcription by Sonnet.

use std::path::Path;
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::claude::{self, Answer, ClaudeCall, ClaudeError};
use crate::content::Symbol;
use crate::figure::PageBox;
use crate::schema::reply_schema;

const COPY_MODEL: &str = "haiku";
const COPY_INSTRUCTION: &str = "Copy the text of the page in the file";
const COPY_PROMPT: &str = include_str!("prompts/copy.md");

// Pinned: the prompts were tuned against this model.
const TRANSCRIBE_MODEL: &str = "claude-sonnet-5-5";
const TRANSCRIBE_EFFORT: &str = "medium";
const TRANSCRIBE_INSTRUCTION: &str = "Transcribe the page in the file";
const TRANSCRIBE_PROMPT: &str = include_str!("prompts/transcribe.md");

const REPLY_TIMEOUT: Duration = Duration::from_secs(180);

// The wording both replies share, written once so the copy and the transcription can never
// describe the same field differently. The model reads these, so changing one changes the answers.
const PIECE_NUMBER: &str = "The piece's place in reading order on this page: 1 for the first piece, 2 for the next, with no gaps.";
const PRINTED_PAGE_NUMBER: &str = "The page number printed in the header or footer, exactly as printed. Null when the page shows none. Never a chapter number.";
const RUNNING_HEADER: &str =
    "The running header or footer line without the page number. Null when there is none.";
const PIECES: &str = "Every piece of the page in reading order.";
const STARTS_MID_SENTENCE: &str = "True only when the first text piece begins partway through a sentence that started on the previous page.";
const ENDS_MID_SENTENCE: &str = "True only when the last text piece is cut off by the end of the page before its sentence ends.";
const HEADING: &str =
    "A chapter title, or a section or sub-section heading, standing on its own line.";
const HEADING_RANK: &str =
    "1 for a chapter title, 2 for a section, 3 for a sub-section, 4 for anything lower.";
const HEADING_PRINTED_NUMBER: &str = "The number printed with the heading, such as \"4.2\", or the chapter number on a chapter's opening page. Null when it has none.";
const HEADING_TEXT: &str = "The heading's exact words, without its printed number.";
const TEXT: &str = "One paragraph of running text, or one whole list.";
const CITES: &str = "Every figure, table and numbered equation this piece refers to by its printed label, on this page or elsewhere. Empty when there are none.";
const CAPTION: &str =
    "The printed title or caption that goes with the label. Null when none is printed.";
const FOOTNOTE: &str = "One footnote or endnote printed on the page.";
const FOOTNOTE_MARKER: &str =
    "The footnote's printed marker, such as \"1\" or \"*\". Null when it has none.";
const FOOTNOTE_MARKDOWN: &str =
    "The footnote's exact printed words without its marker, written like a text piece.";

/// A page broken into its pieces, as Sonnet writes it. The field order is the order the model
/// writes in, so it is part of the prompt.
///
/// Each `description` is the wording the model reads for that field, so changing one changes the
/// answers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
// Do not remove: without it the doc comment above is sent to the model as the description.
#[schemars(description = "One page of a book, broken into its pieces in reading order.")]
pub struct TranscribedPage {
    #[schemars(description = PRINTED_PAGE_NUMBER)]
    pub printed_page_number: Option<String>,
    #[schemars(description = RUNNING_HEADER)]
    pub running_header: Option<String>,
    #[schemars(description = PIECES)]
    pub pieces: Vec<TranscribedPiece>,
    #[schemars(
        description = "Which text or footnote pieces talk about which figure or table on this page. Written after all the pieces."
    )]
    pub discusses: Vec<Discussion>,
    #[schemars(description = STARTS_MID_SENTENCE)]
    pub starts_mid_sentence: bool,
    #[schemars(description = ENDS_MID_SENTENCE)]
    pub ends_mid_sentence: bool,
}

/// One piece of a transcribed page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "kebab-case",
    deny_unknown_fields
)]
// Do not remove: without it the doc comment above is sent to the model as the description.
#[schemars(description = "One piece of a transcribed page.")]
// SMELL: every kind of piece is handled kind by kind in many files: cleaning, the reply check,
// naming a piece in a fault, word counting and saving. A new kind means editing each one. The
// compiler points at the full matches, but not at the places that test for one kind only.
pub enum TranscribedPiece {
    #[schemars(description = HEADING)]
    Heading {
        #[schemars(description = PIECE_NUMBER)]
        number: u32,
        #[schemars(description = HEADING_RANK, range(min = 1, max = 4))]
        rank: u8,
        #[schemars(description = HEADING_PRINTED_NUMBER)]
        printed_number: Option<String>,
        #[schemars(description = HEADING_TEXT)]
        text: String,
    },
    #[schemars(description = TEXT)]
    Text {
        #[schemars(description = PIECE_NUMBER)]
        number: u32,
        #[schemars(
            description = "The paragraph's exact printed words on one line (a list: one item per line), with *italic*, **bold**, inline math between \\( and \\) (bold letters as \\boldsymbol), and footnote markers as [^1]."
        )]
        markdown: String,
        #[schemars(description = CITES)]
        cites: Vec<CitedLabel>,
    },
    #[schemars(
        description = "One displayed formula: an equation or expression set on its own line or lines."
    )]
    Formula {
        #[schemars(description = PIECE_NUMBER)]
        number: u32,
        #[schemars(
            description = "The formula alone as LaTeX on one line: no delimiters, no label, and without the punctuation that closes the sentence after it. Every bold letter, however small, is written with \\boldsymbol."
        )]
        latex: String,
        #[schemars(
            description = "The label printed beside the formula, exactly as printed, such as \"(2.14)\". Null when there is none."
        )]
        label: Option<String>,
        #[schemars(
            description = "The name the page gives this formula or the result it states. Null when the page gives none."
        )]
        name: Option<String>,
        #[schemars(
            description = "One or two plain sentences, with no symbols, saying what the formula expresses in the page's own terms."
        )]
        statement: String,
        #[schemars(
            description = "Each symbol in the formula whose meaning this page states. Empty when the page explains none."
        )]
        symbols: Vec<Symbol>,
    },
    #[schemars(
        description = "One chart, graph, diagram, drawing or photograph, with its label and caption."
    )]
    Figure {
        #[schemars(description = PIECE_NUMBER)]
        number: u32,
        #[schemars(
            description = "The printed label, such as \"Figure 3-2\", with a plain hyphen between numbers. Null when none is printed."
        )]
        label: Option<String>,
        #[schemars(description = CAPTION)]
        caption: Option<String>,
        #[schemars(
            description = "Every separate piece of text printed inside the figure, exactly as printed, one entry each; the tick labels of one axis as one entry."
        )]
        printed_text: Vec<String>,
        #[schemars(
            description = "The smallest rectangle in the picture of the page that holds the whole figure, with its axis titles, tick values, legend, annotations, label and caption. The figure's own image is cut out of the picture along it."
        )]
        bounds: PageBox,
        #[schemars(
            description = "A detailed description for someone who cannot see the figure: what it is, its axes or parts, each curve or part and its shape, each annotation quoted in full, and what the figure demonstrates."
        )]
        explanation: String,
    },
    #[schemars(
        description = "One table: data printed in rows and columns, with its label and caption."
    )]
    Table {
        #[schemars(description = PIECE_NUMBER)]
        number: u32,
        #[schemars(
            description = "The printed label, even when it says \"Figure\". Null when none is printed."
        )]
        label: Option<String>,
        #[schemars(description = CAPTION)]
        caption: Option<String>,
        #[schemars(
            description = "The table as a GitHub Markdown table: a header row, a separator row, then one line per printed row, every line with the same number of cells."
        )]
        markdown: String,
        #[schemars(
            description = "Any note, source line or key printed directly under the table. Null when there is none."
        )]
        note: Option<String>,
        #[schemars(
            description = "One sentence saying what the table records: what its rows are and what its columns are."
        )]
        summary: String,
    },
    #[schemars(description = FOOTNOTE)]
    Footnote {
        #[schemars(description = PIECE_NUMBER)]
        number: u32,
        #[schemars(description = FOOTNOTE_MARKER)]
        marker: Option<String>,
        #[schemars(description = FOOTNOTE_MARKDOWN)]
        markdown: String,
        #[schemars(description = CITES)]
        cites: Vec<CitedLabel>,
    },
}

/// A printed label a text or footnote piece points at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
// Do not remove: without it the doc comment above is sent to the model as the description.
#[schemars(description = "A figure, table or equation cited by its printed label.")]
pub struct CitedLabel {
    #[schemars(
        description = "Follows the printed word: figure, table, or equation for a bracketed equation number."
    )]
    pub kind: CitedKind,
    #[schemars(
        description = "The label written the way the cited item is itself labelled, such as \"Figure 3-2\", \"Table 5.1\" or \"(2.14)\", with a plain hyphen between numbers."
    )]
    pub label: String,
}

// No doc comments on the variants: schemars would turn the simple list of values into a list of
// described constants. There is no footnote value because `ocr` writes those itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CitedKind {
    Figure,
    Table,
    Equation,
}

/// A text or footnote piece that talks about a figure or table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
// Do not remove: without it the doc comment above is sent to the model as the description.
#[schemars(description = "A text or footnote piece that talks about a figure or table.")]
pub struct Discussion {
    #[schemars(
        description = "The number of the text or footnote piece that talks about the figure or table."
    )]
    pub piece: u32,
    #[schemars(description = "The number of the figure or table piece it talks about.")]
    pub about: u32,
}

/// A page copied by Haiku, or the flag that says the page needs the stronger model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
// Do not remove: without it the doc comment above is sent to the model as the description.
#[schemars(
    description = "The plain text of one page of a book, copied exactly, or a flag that the page needs the stronger model."
)]
pub struct CopiedPage {
    #[schemars(
        description = "True when the page shows math notation, an equation or expression on its own line, a figure or a table. Then no pieces are returned."
    )]
    pub needs_stronger_model: bool,
    #[schemars(description = PRINTED_PAGE_NUMBER)]
    pub printed_page_number: Option<String>,
    #[schemars(description = RUNNING_HEADER)]
    pub running_header: Option<String>,
    #[schemars(description = PIECES)]
    pub pieces: Vec<CopiedPiece>,
    #[schemars(description = STARTS_MID_SENTENCE)]
    pub starts_mid_sentence: bool,
    #[schemars(description = ENDS_MID_SENTENCE)]
    pub ends_mid_sentence: bool,
}

/// One piece of a copied page. There is no formula, figure or table: those pages go to Sonnet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "kebab-case",
    deny_unknown_fields
)]
// Do not remove: without it the doc comment above is sent to the model as the description.
#[schemars(
    description = "One piece of a copied page. There is no formula, figure or table: those pages go to Sonnet."
)]
pub enum CopiedPiece {
    #[schemars(description = HEADING)]
    Heading {
        #[schemars(description = PIECE_NUMBER)]
        number: u32,
        #[schemars(description = HEADING_RANK, range(min = 1, max = 4))]
        rank: u8,
        #[schemars(description = HEADING_PRINTED_NUMBER)]
        printed_number: Option<String>,
        #[schemars(description = HEADING_TEXT)]
        text: String,
    },
    #[schemars(description = TEXT)]
    Text {
        #[schemars(description = PIECE_NUMBER)]
        number: u32,
        #[schemars(
            description = "The paragraph's exact printed words on one line (a list: one item per line), with *italic*, **bold**, and footnote markers as [^1]."
        )]
        markdown: String,
        #[schemars(description = CITES)]
        cites: Vec<CitedLabel>,
    },
    #[schemars(description = FOOTNOTE)]
    Footnote {
        #[schemars(description = PIECE_NUMBER)]
        number: u32,
        #[schemars(description = FOOTNOTE_MARKER)]
        marker: Option<String>,
        #[schemars(description = FOOTNOTE_MARKDOWN)]
        markdown: String,
        #[schemars(description = CITES)]
        cites: Vec<CitedLabel>,
    },
}

impl TranscribedPiece {
    /// The piece's place in reading order on its page, whatever its kind.
    pub fn number(&self) -> u32 {
        match self {
            Self::Heading { number, .. }
            | Self::Text { number, .. }
            | Self::Formula { number, .. }
            | Self::Figure { number, .. }
            | Self::Table { number, .. }
            | Self::Footnote { number, .. } => *number,
        }
    }

    /// The kind as it is spelled in a reply and in `page.json`.
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Heading { .. } => "heading",
            Self::Text { .. } => "text",
            Self::Formula { .. } => "formula",
            Self::Figure { .. } => "figure",
            Self::Table { .. } => "table",
            Self::Footnote { .. } => "footnote",
        }
    }
}

impl From<CopiedPage> for TranscribedPage {
    /// Drops the stronger-model flag. A copied page has no `discusses` links because it has no
    /// figure or table.
    fn from(copy: CopiedPage) -> Self {
        let pieces = copy
            .pieces
            .into_iter()
            .map(|piece| match piece {
                CopiedPiece::Heading {
                    number,
                    rank,
                    printed_number,
                    text,
                } => TranscribedPiece::Heading {
                    number,
                    rank,
                    printed_number,
                    text,
                },
                CopiedPiece::Text {
                    number,
                    markdown,
                    cites,
                } => TranscribedPiece::Text {
                    number,
                    markdown,
                    cites,
                },
                CopiedPiece::Footnote {
                    number,
                    marker,
                    markdown,
                    cites,
                } => TranscribedPiece::Footnote {
                    number,
                    marker,
                    markdown,
                    cites,
                },
            })
            .collect();
        Self {
            printed_page_number: copy.printed_page_number,
            running_header: copy.running_header,
            pieces,
            discusses: Vec::new(),
            starts_mid_sentence: copy.starts_mid_sentence,
            ends_mid_sentence: copy.ends_mid_sentence,
        }
    }
}

/// Asks Haiku to copy the text of a page, or to say the page needs the stronger model. One
/// attempt, no retry.
pub(crate) async fn copy_page(page_file: &Path) -> Result<Answer<CopiedPage>, ClaudeError> {
    claude::run(&ClaudeCall {
        model: COPY_MODEL,
        effort: None,
        system_prompt: COPY_PROMPT,
        instruction: COPY_INSTRUCTION,
        correction: None,
        schema: &reply_schema::<CopiedPage>(),
        file: page_file,
        also_read: None,
        timeout: REPLY_TIMEOUT,
    })
    .await
}

/// Asks Sonnet to break the page in `page_file` into its pieces. One attempt, no retry.
///
/// `page_file` is a one-page PDF or an image of a page. `correction` says why the last reply was
/// rejected, for a second try; it has no closing full stop.
///
/// # Errors
/// The errors of one `claude` call: [`ClaudeError::ApiKeySet`], [`ClaudeError::FileUnreadable`],
/// [`ClaudeError::Spawn`], [`ClaudeError::TimedOut`], [`ClaudeError::Exited`],
/// [`ClaudeError::UnreadableResponse`], [`ClaudeError::RunFailed`] and
/// [`ClaudeError::UnexpectedReply`].
pub async fn transcribe_page(
    page_file: &Path,
    correction: Option<&str>,
) -> Result<Answer<TranscribedPage>, ClaudeError> {
    transcribe_page_with_picture(page_file, None, correction).await
}

/// Like [`transcribe_page`], and the model also reads `picture`, a sharper image of the same
/// page. The PDF's text layer anchors letters and indices; the picture shows bold weight and
/// small subscripts. `picture` must sit in the same folder as `page_file`.
pub(crate) async fn transcribe_page_with_picture(
    page_file: &Path,
    picture: Option<&Path>,
    correction: Option<&str>,
) -> Result<Answer<TranscribedPage>, ClaudeError> {
    claude::run(&ClaudeCall {
        model: TRANSCRIBE_MODEL,
        effort: Some(TRANSCRIBE_EFFORT),
        system_prompt: TRANSCRIBE_PROMPT,
        instruction: TRANSCRIBE_INSTRUCTION,
        correction,
        schema: &reply_schema::<TranscribedPage>(),
        file: page_file,
        also_read: picture,
        timeout: REPLY_TIMEOUT,
    })
    .await
}
