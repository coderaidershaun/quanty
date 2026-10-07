//! The text that the model reads: the question, then each item that was found, numbered in the
//! order of the results.

use rag_core::{ItemKind, ItemPayload};

use crate::search::{SearchResults, page_text};

/// The question, then one block for each item, with a blank line between them.
pub(super) fn input_for(question: &str, results: &SearchResults) -> String {
    let items = results
        .hits
        .iter()
        .enumerate()
        .map(|(index, hit)| item_block(index + 1, &hit.item.payload));
    std::iter::once(format!("Question: {question}"))
        .chain(items)
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn item_block(number: usize, payload: &ItemPayload) -> String {
    let mut lines = vec![
        format!("Item {number}"),
        format!("document: {}", payload.doc_title),
        format!("page: {}", page_text(payload)),
        format!("kind: {}", payload.kind.as_str()),
    ];
    if let Some(label) = &payload.label {
        lines.push(format!("label: {label}"));
    }
    if let Some(picture) = &payload.image_path {
        lines.push(format!("picture: {}", picture.display()));
    }
    lines.push(body_heading(payload.kind).to_owned());
    lines.push(payload.text.clone());
    lines.join("\n")
}

/// What the stored text of an item is, in the words the model is told in its prompt.
fn body_heading(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Chunk => "text:",
        ItemKind::Formula => "latex:",
        ItemKind::Figure => "explanation:",
        ItemKind::Table => "table:",
    }
}
