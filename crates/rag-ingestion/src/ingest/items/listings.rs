//! The code listings, one item each, with the parts that page breaks cut joined again as the
//! chapter lists them.

use ocr::{Chapter, ChapterItem, ChapterPiece, PieceDetail};
use rag_core::{DocumentInput, ItemKind};

use super::pieces::{join_blocks, label_line, printed_label};
use super::{Draft, context_line};

pub(super) fn listing_drafts(chapter: &Chapter, doc_title: &str) -> Vec<Draft> {
    chapter
        .items()
        .iter()
        .filter_map(|item| listing_draft(item, doc_title))
        .collect()
}

/// `None` for an item that is not a code listing.
fn listing_draft(listing: &ChapterItem<'_>, doc_title: &str) -> Option<Draft> {
    let (label, caption, _) = code_fields(listing.first)?;
    let code = listing.content();
    let language = listing.language();
    let heading = label_line(label, caption);
    let language_line = language.map(|name| format!("Language: {name}"));
    let summaries = listing
        .parts()
        .filter_map(code_fields)
        .map(|(_, _, summary)| Some(summary));
    let blocks = [heading.as_deref(), language_line.as_deref()]
        .into_iter()
        .chain(summaries)
        .chain([Some(code.as_str())]);
    let text = join_blocks(blocks);

    Some(Draft {
        first_piece: listing.first.id,
        printed_page: listing.first.printed_page_number.clone(),
        last_page: listing.last_page(),
        last_printed_page: listing.last_printed_page().map(str::to_owned),
        kind: ItemKind::Code,
        text: code,
        label: printed_label(label),
        language: language.map(str::to_owned),
        cites: Vec::new(),
        input: DocumentInput {
            title: context_line(doc_title, &listing.first.section),
            text,
            image: None,
        },
    })
}

/// The label, the caption and the summary of a code piece. `None` for any other piece.
fn code_fields(piece: &ChapterPiece) -> Option<(Option<&str>, Option<&str>, &str)> {
    let PieceDetail::Code {
        label,
        caption,
        summary,
        ..
    } = &piece.detail
    else {
        return None;
    };
    Some((label.as_deref(), caption.as_deref(), summary))
}
