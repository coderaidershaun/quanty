//! The `chapter_concepts` tool: the concepts that one document discusses, or that one of its pages
//! does, as the ingest found them in the graph.

use graph::{ConceptNode, FalkorGraph};
use rag_core::Config;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::LibraryError;
use super::chapter::parse_document_id;

/// What an agent gives `chapter_concepts`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct ChapterConceptsArgs {
    /// From `list_documents` or a search result.
    document_id: String,
    /// One page only: the `page` of a search result or of `chapter_contents` (from 1), not the
    /// number printed on the page. The whole document when left out.
    page: Option<u32>,
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
}

#[derive(Serialize, JsonSchema)]
struct ConceptView {
    name: String,
    /// What the concept is, in one line.
    definition: String,
}

impl From<ConceptNode> for ConceptView {
    fn from(concept: ConceptNode) -> ConceptView {
        ConceptView {
            name: concept.name,
            definition: concept.definition,
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
    let graph = FalkorGraph::connect(config).await?;
    let document = graph
        .document(id)
        .await?
        .ok_or(LibraryError::NotStored { id })?;
    let concepts = match args.page {
        Some(page) => graph.concepts_on_page(id, page).await?,
        None => graph.concepts_in_document(id).await?,
    };
    Ok(ConceptsView {
        document_id: id.to_string(),
        title: document.title,
        page: args.page,
        concepts: concepts.into_iter().map(ConceptView::from).collect(),
    })
}
