//! What is saved about one piece of a page: its entry in `page.json`, the fields each kind
//! keeps, and the links between pieces.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{FigureImage, PageBox};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PieceEntry {
    /// 1, 2, 3 and so on, with no gaps.
    pub number: u32,
    pub file: String,
    #[serde(flatten)]
    pub detail: PieceDetail,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "kebab-case"
)]
pub enum PieceDetail {
    Heading {
        /// 1 for a chapter title, down to 4.
        rank: u8,
        #[serde(default)]
        printed_number: Option<String>,
    },
    Text {
        #[serde(default)]
        cites: Vec<Cite>,
    },
    Formula {
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        name: Option<String>,
        statement: String,
        #[serde(default)]
        symbols: Vec<Symbol>,
    },
    Figure {
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        caption: Option<String>,
        /// Every separate piece of text printed inside the figure.
        #[serde(default)]
        printed_text: Vec<String>,
        /// The rectangle the model gave for the figure, before any padding, kept even when it
        /// was unusable. Trust it only when the figure's image shows the figure.
        #[serde(default)]
        bounds: Option<PageBox>,
        /// The picture saved for the figure: its own, or the whole page when it could not be cut.
        #[serde(default)]
        image: Option<FigureImage>,
    },
    Table {
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        caption: Option<String>,
        summary: String,
    },
    Footnote {
        #[serde(default)]
        marker: Option<String>,
        #[serde(default)]
        cites: Vec<Cite>,
    },
}

impl PieceDetail {
    /// The kind as it is spelled in `page.json` and in the piece's file name.
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

    /// `tex` for a formula, so its file opens as LaTeX. Every other kind is Markdown.
    pub fn file_extension(&self) -> &'static str {
        match self {
            Self::Formula { .. } => "tex",
            _ => "md",
        }
    }
}

impl PieceEntry {
    /// The file name is built from the number and kind, such as `03-formula.tex`.
    pub fn new(number: u32, detail: PieceDetail) -> Self {
        let file = format!(
            "{number:02}-{}.{}",
            detail.kind_name(),
            detail.file_extension()
        );
        Self {
            number,
            file,
            detail,
        }
    }
}

/// A symbol of a formula and what the page says it stands for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
// Do not remove: without it the doc comment above is sent to the model as the description.
#[schemars(
    description = "A symbol of a formula and the meaning the page gives it, in the page's own words."
)]
pub struct Symbol {
    #[schemars(description = "The symbol as LaTeX, without delimiters.")]
    pub symbol: String,
    #[schemars(description = "What the page says the symbol stands for, in the page's own words.")]
    pub meaning: String,
}

/// A printed label a piece points at: a figure, a table, an equation or a footnote marker.
///
/// The label is kept as printed. Nothing in `ocr` looks up the piece it points at, on this page
/// or another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cite {
    pub kind: CiteKind,
    pub label: String,
}

/// What a citation points at. The kind follows the printed word, not the kind of the cited piece:
/// "Figure 7-2" is a figure citation even when that item was saved as a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CiteKind {
    Figure,
    Table,
    Equation,
    Footnote,
}

/// A link between two pieces of one page, by piece number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Relationship {
    pub kind: RelationshipKind,
    pub from: u32,
    pub to: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RelationshipKind {
    /// A text piece leads into a formula.
    Introduces,
    /// A text or footnote piece talks about a figure or table.
    Discusses,
    /// A footnote belongs to the piece that carries its marker.
    FootnoteOf,
}
