//! A piece of a converted chapter as an agent reads it.

use ocr::{ChapterItem, ChapterPiece, PieceDetail};
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Serialize, JsonSchema)]
pub(super) struct PieceView {
    /// The place of the piece on the page, from 1.
    number: u32,
    /// `heading`, `text`, `formula`, `figure`, `table`, `footnote` or `code`.
    kind: String,
    /// The headings the piece sits under, outermost first.
    section: Vec<String>,
    /// The label as printed, such as "(7.3)". Formulas, figures, tables and code only.
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    /// The caption as printed, without its label. Figures, tables and code only, and left out
    /// when there is none.
    #[serde(skip_serializing_if = "Option::is_none")]
    caption: Option<String>,
    /// The name the book gives the formula, such as "Black–Scholes call price". Formulas only, and
    /// left out when the book names none.
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    /// The language of a code listing in lower case, such as "python". Left out when the language
    /// is not known.
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    /// The piece as saved: Markdown, LaTeX for a formula, or the code as printed for code.
    content: String,
    /// The picture URL relative to the app base, contained beneath content/.
    #[serde(skip_serializing_if = "Option::is_none")]
    picture: Option<String>,
}

impl PieceView {
    pub(super) fn from_piece(piece: &ChapterPiece, content: &std::path::Path) -> PieceView {
        PieceView {
            number: piece.id.number,
            kind: piece.detail.kind_name().to_owned(),
            section: piece
                .section
                .iter()
                .map(|heading| heading.text.clone())
                .collect(),
            label: label_of(&piece.detail),
            caption: caption_of(&piece.detail),
            name: name_of(&piece.detail),
            language: language_of(&piece.detail),
            content: piece.content.clone(),
            picture: piece
                .figure_image
                .as_ref()
                .and_then(|picture| rag_core::content_url(&picture.path, content)),
        }
    }
}

impl PieceView {
    /// An item of a chapter as one piece: the first piece, with the content and the language of
    /// the whole item, which is longer than the first piece for a listing that page breaks cut.
    pub(super) fn of_item(item: &ChapterItem<'_>, content: &std::path::Path) -> PieceView {
        PieceView {
            content: item.content(),
            language: item.language().map(str::to_owned),
            ..PieceView::from_piece(item.first, content)
        }
    }
}

fn label_of(detail: &PieceDetail) -> Option<String> {
    match detail {
        PieceDetail::Formula { label, .. }
        | PieceDetail::Figure { label, .. }
        | PieceDetail::Table { label, .. }
        | PieceDetail::Code { label, .. } => label.clone(),
        PieceDetail::Heading { .. } | PieceDetail::Text { .. } | PieceDetail::Footnote { .. } => {
            None
        }
    }
}

fn caption_of(detail: &PieceDetail) -> Option<String> {
    match detail {
        PieceDetail::Figure { caption, .. }
        | PieceDetail::Table { caption, .. }
        | PieceDetail::Code { caption, .. } => caption.clone(),
        _ => None,
    }
}

fn name_of(detail: &PieceDetail) -> Option<String> {
    match detail {
        PieceDetail::Formula { name, .. } => name.clone(),
        _ => None,
    }
}

fn language_of(detail: &PieceDetail) -> Option<String> {
    match detail {
        PieceDetail::Code { language, .. } => language.clone(),
        _ => None,
    }
}
