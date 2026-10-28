//! The `compare_documents` and `document_concepts` tools: the concepts that several documents
//! share and the concepts only one of them has, and the concepts of one document or page.

use std::collections::{BTreeMap, HashMap};

use graph::{ConceptNode, FalkorGraph};
use rag_core::{ConceptId, Config, DocId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::args::{bounded, parse_document, stored_title};
use super::error::ConceptsError;
use crate::library::LibraryError;

const FEWEST_DOCUMENTS: usize = 2;
const MOST_DOCUMENTS: usize = 10;
const DEFAULT_LIMIT: u32 = 50;
const MOST_COMPARED: u32 = 500;

/// What an agent gives `compare_documents`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct CompareDocumentsArgs {
    /// From 2 to 10 different `document_id`s from `list_documents`.
    document_ids: Vec<String>,
    /// How many concepts at most in each list, from 1 to 500. Default 50.
    limit: Option<u32>,
}

#[derive(Serialize, JsonSchema)]
pub(crate) struct CompareResult {
    documents: Vec<DocumentTitle>,
    /// Concepts that at least two of the documents mention, the most mentioned first.
    shared: Vec<SharedView>,
    /// For each document, the concepts that only it mentions.
    only_in: BTreeMap<String, Vec<OnlyView>>,
    /// True when `limit` cut a list.
    truncated: bool,
}

#[derive(Serialize, JsonSchema)]
struct DocumentTitle {
    document_id: String,
    title: String,
}

#[derive(Serialize, JsonSchema)]
struct SharedView {
    concept_id: String,
    name: String,
    /// For each document that mentions the concept, how many of its items do.
    mentions: BTreeMap<String, u64>,
}

#[derive(Serialize, JsonSchema)]
struct OnlyView {
    concept_id: String,
    name: String,
    mentions: u64,
}

/// What an agent gives `document_concepts`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct DocumentConceptsArgs {
    /// From `list_documents` or a search result.
    document_id: String,
    /// One page only: the `page` of a search result (from 1). The whole document when left out.
    page: Option<u32>,
    /// How many concepts at most, from 1 to 500. All when left out.
    limit: Option<u32>,
    /// Add `mentions`, the number of items of the whole document that mention each concept.
    with_counts: Option<bool>,
}

#[derive(Serialize, JsonSchema)]
pub(crate) struct DocumentConceptsResult {
    document_id: String,
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    page: Option<u32>,
    /// The most mentioned first.
    concepts: Vec<DocumentConcept>,
}

#[derive(Serialize, JsonSchema)]
struct DocumentConcept {
    concept_id: String,
    name: String,
    definition: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    mentions: Option<u64>,
}

impl CompareDocumentsArgs {
    fn read(self) -> Result<(Vec<DocId>, usize), ConceptsError> {
        let ids = self
            .document_ids
            .iter()
            .map(|text| parse_document(text))
            .collect::<Result<Vec<_>, _>>()?;
        let mut distinct = ids.clone();
        distinct.sort();
        distinct.dedup();
        if distinct.len() != ids.len() || !(FEWEST_DOCUMENTS..=MOST_DOCUMENTS).contains(&ids.len())
        {
            return Err(ConceptsError::DocumentCount {
                given: ids.len(),
                distinct: distinct.len(),
            });
        }
        let limit = bounded("limit", self.limit, 1..=MOST_COMPARED, DEFAULT_LIMIT)?;
        Ok((ids, limit))
    }
}

/// The concepts that the documents share, and those that only one has.
///
/// # Errors
/// A bad argument, a document that is not stored, or a graph that is not ready.
pub(crate) async fn compare_documents(
    config: &Config,
    args: CompareDocumentsArgs,
) -> Result<CompareResult, ConceptsError> {
    let (ids, limit) = args.read()?;
    let graph = FalkorGraph::connect(config).await?;
    let documents = document_titles(&graph, &ids).await?;

    let mut shared = Vec::new();
    let mut only_in: BTreeMap<String, Vec<OnlyView>> = documents
        .iter()
        .map(|document| (document.document_id.clone(), Vec::new()))
        .collect();
    let mut truncated = false;
    // The graph gives the concepts with the most mentions first, then by name, and each list
    // keeps that order.
    for found in graph.concepts_shared(&ids).await? {
        let concept_id = found.concept.id.to_string();
        let name = found.concept.name;
        let was_cut = match found.mentions.as_slice() {
            [(document, mentions)] => {
                let only = OnlyView {
                    concept_id,
                    name,
                    mentions: *mentions,
                };
                let list = only_in.entry(document.to_string()).or_default();
                push_unless_full(list, limit, only)
            }
            counts => {
                let mentions = counts
                    .iter()
                    .map(|(document, mentions)| (document.to_string(), *mentions))
                    .collect();
                let concept = SharedView {
                    concept_id,
                    name,
                    mentions,
                };
                push_unless_full(&mut shared, limit, concept)
            }
        };
        truncated |= was_cut;
    }
    Ok(CompareResult {
        documents,
        shared,
        only_in,
        truncated,
    })
}

/// The concepts of a document, or of one page of it, as `chapter_concepts` gives them, with the
/// ids of the concepts, and with how many items of the whole document mention each on request.
///
/// # Errors
/// A bad argument, a document that is not stored, or a graph that is not ready.
pub(crate) async fn document_concepts(
    config: &Config,
    args: DocumentConceptsArgs,
) -> Result<DocumentConceptsResult, ConceptsError> {
    let id = parse_document(&args.document_id)?;
    if args.page == Some(0) {
        return Err(LibraryError::ZeroPage.into());
    }
    let limit = args
        .limit
        .map(|limit| bounded("limit", Some(limit), 1..=MOST_COMPARED, MOST_COMPARED))
        .transpose()?;
    let graph = FalkorGraph::connect(config).await?;
    let title = stored_title(&graph, id).await?;
    let concepts: Vec<ConceptNode> = match args.page {
        Some(page) => graph.concepts_on_page(id, page).await?,
        None => graph.concepts_in_document(id).await?,
    };
    let counts = if args.with_counts.unwrap_or(false) {
        Some(document_counts(&graph, id).await?)
    } else {
        None
    };
    let concepts = concepts
        .into_iter()
        .take(limit.unwrap_or(usize::MAX))
        .map(|concept| DocumentConcept {
            mentions: counts
                .as_ref()
                .map(|counts| counts.get(&concept.id).copied().unwrap_or(0)),
            concept_id: concept.id.to_string(),
            name: concept.name,
            definition: concept.definition,
        })
        .collect();
    Ok(DocumentConceptsResult {
        document_id: id.to_string(),
        title,
        page: args.page,
        concepts,
    })
}

async fn document_titles(
    graph: &FalkorGraph,
    ids: &[DocId],
) -> Result<Vec<DocumentTitle>, ConceptsError> {
    let mut documents = Vec::with_capacity(ids.len());
    for id in ids {
        let title = stored_title(graph, *id).await?;
        documents.push(DocumentTitle {
            document_id: id.to_string(),
            title,
        });
    }
    Ok(documents)
}

/// Adds the row unless the list already holds `limit` rows. True when the row was left out.
fn push_unless_full<T>(list: &mut Vec<T>, limit: usize, row: T) -> bool {
    let is_full = list.len() == limit;
    if !is_full {
        list.push(row);
    }
    is_full
}

/// How many items of the document mention each concept.
async fn document_counts(
    graph: &FalkorGraph,
    document: DocId,
) -> Result<HashMap<ConceptId, u64>, ConceptsError> {
    let shared = graph.concepts_shared(&[document]).await?;
    Ok(shared
        .into_iter()
        .map(|found| {
            let mentions = found.mentions.iter().map(|(_, count)| count).sum();
            (found.concept.id, mentions)
        })
        .collect())
}
