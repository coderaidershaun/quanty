//! What one exported file holds, and the kinds of file it can be written as. A document is not
//! tied to what its text is about: an answer and a cheat sheet are both a title, blocks and the
//! sources they cite.

use std::path::PathBuf;

/// The longest generated file name before its extension.
const FILE_NAME_LENGTH: usize = 80;

/// What one file holds, in the order it holds it.
#[derive(
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Default,
)]
pub struct Document {
    pub title: String,
    /// The one line under the title that says where the text comes from, such as the question an
    /// answer answers, or the chapter a cheat sheet is drawn from. Empty means no line.
    pub origin: String,
    pub blocks: Vec<Block>,
    /// The sources the blocks cite, listed at the end under "Sources". Empty means no list.
    pub sources: Vec<Source>,
}

/// Every `caption` is one line of Markdown that may hold `**label**`, and may be empty. A `cite`
/// is the number of the source the block comes from.
#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub enum Block {
    Heading(String),
    /// CommonMark with GitHub tables, `$…$` and `$$…$$` formulas and footnote marks. `cites` are
    /// the numbers of the sources it cites, written after its last paragraph as `[n]`.
    Text {
        markdown: String,
        cites: Vec<usize>,
    },
    /// One-line points, each a line of Markdown.
    List(Vec<String>),
    /// A formula set apart: LaTeX with no delimiters, a label such as "(2.4)" at the right, and a
    /// caption under it.
    Formula {
        latex: String,
        label: Option<String>,
        caption: String,
        cite: Option<usize>,
    },
    /// A picture from a PNG or JPEG file. `description` stands in for the picture when there is
    /// none or the file cannot be read.
    Figure {
        picture: Option<PathBuf>,
        description: String,
        caption: String,
        cite: Option<usize>,
    },
    /// A GitHub Markdown table. Its caption stands above it.
    Table {
        markdown: String,
        caption: String,
        cite: Option<usize>,
    },
    Code {
        code: String,
        language: Option<String>,
        caption: String,
        cite: Option<usize>,
    },
}

/// The first word of the language of a code listing: the only part a Markdown fence line can
/// hold, and so the name that every kind of file shows.
pub(crate) fn language_word(language: Option<&str>) -> &str {
    language
        .and_then(|language| language.split_whitespace().next())
        .unwrap_or("")
}

#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub struct Source {
    pub number: usize,
    /// The document and the page as printed, such as "Hull, chapter 2: Options, p. 7 — Figure 2.1".
    pub text: String,
}

/// The kinds of file a document is written as.
#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub enum Format {
    Pdf,
    Markdown,
    Html,
}

impl Format {
    pub const ALL: [Format; 3] = [Format::Pdf, Format::Markdown, Format::Html];

    pub fn name(self) -> &'static str {
        match self {
            Format::Pdf => "PDF",
            Format::Markdown => "Markdown",
            Format::Html => "HTML",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Format::Pdf => "pdf",
            Format::Markdown => "md",
            Format::Html => "html",
        }
    }

    /// Builds a file name with at most 80 characters before its extension, removing forbidden
    /// characters. Uses `Document` when nothing is left.
    pub fn file_name(self, words: &str) -> String {
        let spaced: String = words
            .chars()
            .filter(|character| !r#"/\:*?"<>|"#.contains(*character))
            .map(|character| {
                if character.is_whitespace() {
                    ' '
                } else {
                    character
                }
            })
            .filter(|character| !character.is_control())
            .collect();
        let single_spaced = spaced.split_whitespace().collect::<Vec<_>>().join(" ");
        let cut: String = single_spaced.chars().take(FILE_NAME_LENGTH).collect();
        let stem = cut.trim_matches(|character: char| character == '.' || character == ' ');
        let stem = if stem.is_empty() { "Document" } else { stem };
        format!("{stem}.{}", self.extension())
    }
}
