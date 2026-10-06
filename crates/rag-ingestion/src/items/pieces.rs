//! The pieces that are one item each: a formula, a figure or a table.

use ocr::{Chapter, ChapterPiece, PieceDetail, RelationshipKind, Symbol};
use rag_core::{DocumentInput, ItemKind};

use super::{BLOCK_SEPARATOR, Draft, context_line};

/// `None` for a piece that is not a formula, a figure or a table.
pub(super) fn piece_draft(
    chapter: &Chapter,
    piece: &ChapterPiece,
    doc_title: &str,
) -> Option<Draft> {
    let (kind, text, image) = match &piece.detail {
        PieceDetail::Formula {
            label,
            name,
            statement,
            symbols,
        } => {
            let heading = label_line(name.as_deref(), label.as_deref());
            let symbols = symbols_line(symbols);
            let text = join_blocks([
                lead_in(chapter, piece),
                Some(&piece.content),
                heading.as_deref(),
                Some(statement),
                symbols.as_deref(),
            ]);
            (ItemKind::Formula, text, None)
        }
        PieceDetail::Table {
            label,
            caption,
            summary,
        } => {
            let heading = label_line(label.as_deref(), caption.as_deref());
            let text = join_blocks([heading.as_deref(), Some(summary), Some(&piece.content)]);
            (ItemKind::Table, text, None)
        }
        PieceDetail::Figure { label, caption, .. } => {
            let heading = label_line(label.as_deref(), caption.as_deref());
            let text = join_blocks([heading.as_deref(), Some(&piece.content)]);
            let picture = piece
                .figure_image
                .as_ref()
                .map(|picture| picture.path.clone());
            (ItemKind::Figure, text, picture)
        }
        _ => return None,
    };
    Some(Draft {
        first_piece: piece.id,
        printed_page: piece.printed_page_number.clone(),
        kind,
        text: piece.content.clone(),
        input: DocumentInput {
            title: context_line(doc_title, &piece.section),
            text,
            image,
        },
    })
}

/// The whole text piece that introduces the formula. When several do, the latest one is the one
/// right before the formula.
fn lead_in<'a>(chapter: &'a Chapter, formula: &ChapterPiece) -> Option<&'a str> {
    formula
        .relationships
        .iter()
        .filter(|link| link.kind == RelationshipKind::Introduces && link.to == formula.id)
        .map(|link| link.from)
        .max()
        .and_then(|id| chapter.piece(id))
        .map(|piece| piece.content.as_str())
}

fn join_blocks<'a>(parts: impl IntoIterator<Item = Option<&'a str>>) -> String {
    parts
        .into_iter()
        .flatten()
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>()
        .join(BLOCK_SEPARATOR)
}

/// The parts that exist, joined with a space: a name and a label, or a label and a caption.
fn label_line(first: Option<&str>, second: Option<&str>) -> Option<String> {
    let line = [first, second]
        .into_iter()
        .flatten()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    (!line.is_empty()).then_some(line)
}

fn symbols_line(symbols: &[Symbol]) -> Option<String> {
    if symbols.is_empty() {
        return None;
    }
    let meanings: Vec<String> = symbols
        .iter()
        .map(|symbol| format!("{}: {}", symbol.symbol, symbol.meaning))
        .collect();
    Some(format!("Symbols: {}", meanings.join("; ")))
}
