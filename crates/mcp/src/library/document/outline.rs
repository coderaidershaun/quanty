//! The `document_outline` tool: the sections of a converted chapter, with the pages each covers,
//! what it holds, and how many concepts its items mention.

use std::collections::HashMap;

use graph::FalkorGraph;
use rag_core::{Config, DocId, ItemId, ItemKind, error_chain};
use rag_ingestion::document_title;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::mentions::DocumentConcepts;
use super::sections::{PATH_SEPARATOR, page_range};
use crate::library::LibraryError;
use crate::library::chapter::{DocumentHeader, Whole, parse_document_id};
use crate::library::contents::ContentsCounts;
use crate::library::discovery::Discovery;

/// What an agent gives `document_outline`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct DocumentOutlineArgs {
    /// From `list_documents` or a search result.
    document_id: String,
}

/// The sections of one document.
#[derive(Serialize, JsonSchema)]
pub(crate) struct Outline {
    #[serde(flatten)]
    document: DocumentHeader,
    sections: Vec<SectionRow>,
    /// Why `concepts` is left out of every section. Only when the graph could not be read.
    #[serde(skip_serializing_if = "Option::is_none")]
    concepts_error: Option<String>,
    #[serde(flatten)]
    discovery: Discovery,
}

#[derive(Serialize, JsonSchema)]
pub(super) struct SectionRow {
    /// Give it as `section` to `read_section`. Counted from 1.
    index: usize,
    /// The headings above the text of the section, outermost first.
    path: Vec<String>,
    /// The level of the last heading of `path`, from 1 for a chapter title down to 4.
    #[serde(skip_serializing_if = "Option::is_none")]
    rank: Option<u8>,
    /// The number the book prints before that heading.
    #[serde(skip_serializing_if = "Option::is_none")]
    printed_number: Option<String>,
    /// The first and the last page of the section, counted from 1.
    pages: [u32; 2],
    /// The characters of its Markdown.
    chars: usize,
    items: ContentsCounts,
    /// How many different concepts its items mention.
    #[serde(skip_serializing_if = "Option::is_none")]
    concepts: Option<usize>,
}

/// The outline of the chapter that the document id names. The chapter is read from disk, and the
/// graph is read only for the concepts.
///
/// # Errors
/// A bad argument, a document that no converted chapter has, or a chapter that cannot be read.
pub(crate) async fn document_outline(
    config: &Config,
    args: DocumentOutlineArgs,
) -> Result<Outline, LibraryError> {
    let id = parse_document_id(&args.document_id)?;
    let whole = Whole::find(&config.content_folder, id)?;
    let concepts = concepts_of(config, id, &whole).await;
    let rows = section_rows(&whole, concepts.as_ref().ok());
    let title = document_title(&whole.entry.index);
    let discovery = rows.iter().fold(Discovery::new(), |discovery, row| {
        let label = if row.path.is_empty() {
            title.clone()
        } else {
            row.path.join(PATH_SEPARATOR)
        };
        let arguments = json!({ "document_id": id.to_string(), "section": row.index });
        discovery.reading(&label, "read_section", arguments)
    });
    Ok(Outline {
        document: DocumentHeader::of(&whole.entry),
        sections: rows,
        concepts_error: concepts.err(),
        discovery,
    })
}

/// The concepts of the document, or the words of why the graph could not give them.
async fn concepts_of(
    config: &Config,
    id: DocId,
    whole: &Whole,
) -> Result<DocumentConcepts, String> {
    let item_ids: Vec<ItemId> = whole.items.iter().map(|item| item.id).collect();
    let graph = FalkorGraph::connect(config)
        .await
        .map_err(|error| error_chain(&error))?;
    DocumentConcepts::read(&graph, id, &item_ids)
        .await
        .map_err(|error| error_chain(&error))
}

/// The rows of the outline. A row has `concepts` only when the concepts of the document are given.
pub(super) fn section_rows(whole: &Whole, concepts: Option<&DocumentConcepts>) -> Vec<SectionRow> {
    let kinds: HashMap<ItemId, ItemKind> = whole
        .items
        .iter()
        .map(|item| (item.id, item.payload.kind))
        .collect();
    whole
        .sections
        .iter()
        .enumerate()
        .map(|(place, (section, mark))| SectionRow {
            index: place + 1,
            path: section.section_path.clone(),
            rank: mark.as_ref().map(|mark| mark.rank),
            printed_number: mark.as_ref().and_then(|mark| mark.printed_number.clone()),
            pages: page_range(&section.pages),
            chars: section.markdown.chars().count(),
            items: ContentsCounts::of_kinds(
                section
                    .item_ids
                    .iter()
                    .filter_map(|id| kinds.get(id).copied()),
            ),
            concepts: concepts.map(|concepts| concepts.concepts_in(&section.item_ids)),
        })
        .collect()
}
