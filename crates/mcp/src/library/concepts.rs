//! The `chapter_concepts` tool: the concepts that one document discusses, or that one of its pages
//! does, as the ingest found them in the graph.

use graph::{ConceptNode, FalkorGraph};
use rag_core::Config;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::LibraryError;
use super::chapter::parse_document_id;
use crate::args::{Budget, Scope, Slice, Truncation};

/// What an agent gives `chapter_concepts`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct ChapterConceptsArgs {
    /// From `list_documents`.
    document_id: String,
    /// One page only, from 1.
    page: Option<u32>,
    /// Text limit. Default 24000.
    max_chars: Option<u32>,
    /// From a `truncated` result.
    cursor: Option<String>,
}

/// The concepts of a document, or of one page of it, the most mentioned first.
#[derive(Serialize, JsonSchema)]
pub(crate) struct ConceptsView {
    document_id: String,
    /// The whole stored title of the document.
    title: String,
    /// The page the concepts are of. Left out for the whole document.
    #[serde(skip_serializing_if = "Option::is_none")]
    page: Option<u32>,
    concepts: Vec<ConceptView>,
    #[serde(flatten)]
    more: Truncation,
}

#[derive(Serialize, JsonSchema)]
pub(crate) struct ConceptView {
    name: String,
    /// What the concept is, in one line.
    definition: String,
}

impl ConceptView {
    /// The part of a concept that a read took. The name and the definition are counted as one
    /// text whose name is never cut, unless the budget is smaller than the name alone.
    fn taken(concept: &ConceptNode, slice: &Slice) -> ConceptView {
        let name_chars = concept.name.chars().count();
        let in_name = name_chars.saturating_sub(slice.offset);
        ConceptView {
            name: slice.text.chars().take(in_name).collect(),
            definition: slice.text.chars().skip(in_name).collect(),
        }
    }
}

impl From<&ConceptNode> for ConceptView {
    fn from(concept: &ConceptNode) -> ConceptView {
        ConceptView {
            name: concept.name.clone(),
            definition: concept.definition.clone(),
        }
    }
}

/// The concepts of the stored document that the id names, ordered by how many of its items
/// mention them, the most first.
///
/// # Errors
/// A bad argument, a graph that is not ready, or a document that the graph does not hold.
pub(crate) async fn chapter_concepts(
    config: &Config,
    args: ChapterConceptsArgs,
) -> Result<ConceptsView, LibraryError> {
    let id = parse_document_id(&args.document_id)?;
    if args.page == Some(0) {
        return Err(LibraryError::ZeroPage);
    }
    let scope = Scope::of("chapter_concepts").with(id).with(
        args.page
            .map_or("whole".to_owned(), |page| page.to_string()),
    );
    let budget = Budget::new(scope, args.max_chars, args.cursor.as_deref())?;
    let graph = FalkorGraph::connect(config).await?;
    let document = graph
        .document(id)
        .await?
        .ok_or(LibraryError::NotStored { id })?;
    let concepts = match args.page {
        Some(page) => graph.concepts_on_page(id, page).await?,
        None => graph.concepts_in_document(id).await?,
    };
    let texts: Vec<String> = concepts
        .iter()
        .map(|concept| format!("{}{}", concept.name, concept.definition))
        .collect();
    let taken = budget.take_keeping_heads(&texts, |unit| concepts[unit].name.chars().count())?;
    Ok(ConceptsView {
        document_id: id.to_string(),
        title: document.title,
        page: args.page,
        concepts: taken
            .slices
            .iter()
            .map(|slice| ConceptView::taken(&concepts[slice.unit], slice))
            .collect(),
        more: taken.truncation,
    })
}
