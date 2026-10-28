//! The `concept_items` and `similar_items` tools: the items that mention a concept, a page of
//! them at a time, and the stored items nearest to a stored item.

use graph::FalkorGraph;
use rag_core::{Config, ItemFilter, ItemHit, ItemStore};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::args::{
    bounded, offset_of, parse_concept_id, parse_document, parse_item_kind, stored_concept,
    stored_title,
};
use super::error::ConceptsError;
use super::view::ItemRow;
use crate::args::non_blank;

const DEFAULT_ITEMS: u32 = 20;
const MOST_ITEMS: u32 = 100;
const DEFAULT_SIMILAR: u32 = 10;
const MOST_SIMILAR: u32 = 50;

/// What an agent gives `concept_items`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct ConceptItemsArgs {
    /// The `concept_id` that a result gave.
    concept_id: String,
    /// Keep only the items of this document: a `document_id` from `list_documents`.
    document: Option<String>,
    /// Keep only items of this kind: `chunk`, `formula`, `figure`, `table` or `code`.
    kind: Option<String>,
    /// How many items at most, from 1 to 100. Default 20.
    limit: Option<u32>,
    /// The `next_cursor` of the previous result, to read the next page.
    cursor: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub(crate) struct ConceptItemsResult {
    concept_id: String,
    /// By document title, then page.
    items: Vec<ItemRow>,
    /// Give it as `cursor` to read the next page. Left out on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    next_cursor: Option<String>,
}

/// What an agent gives `similar_items`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct SimilarItemsArgs {
    /// The `item_id` that a result gave.
    item_id: String,
    /// How many items at most, from 1 to 50. Default 10.
    limit: Option<u32>,
    /// Keep only items of this kind: `chunk`, `formula`, `figure`, `table` or `code`.
    kind: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub(crate) struct SimilarItemsResult {
    item_id: String,
    /// Nearest first.
    items: Vec<SimilarItem>,
}

#[derive(Serialize, JsonSchema)]
struct SimilarItem {
    item_id: String,
    document_id: String,
    /// The title of the document.
    document: String,
    page: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    printed_page: Option<String>,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    /// How near the item is, from 0 to 1. Higher is nearer.
    score: f32,
}

impl From<ItemHit> for SimilarItem {
    fn from(hit: ItemHit) -> SimilarItem {
        let payload = hit.payload;
        SimilarItem {
            item_id: hit.id.to_string(),
            document_id: payload.doc_id.to_string(),
            document: payload.doc_title,
            page: payload.page,
            printed_page: payload.printed_page,
            kind: payload.kind.as_str().to_owned(),
            label: payload.label,
            score: hit.score,
        }
    }
}

/// A page of the items that mention the concept.
///
/// # Errors
/// A bad argument, a concept or a document that is not stored, or a graph that is not ready.
pub(crate) async fn concept_items(
    config: &Config,
    args: ConceptItemsArgs,
) -> Result<ConceptItemsResult, ConceptsError> {
    let id = parse_concept_id("concept_id", &args.concept_id)?;
    let document = non_blank(args.document)
        .map(|text| parse_document(&text))
        .transpose()?;
    let kind = parse_item_kind(args.kind)?;
    let limit = bounded("limit", args.limit, 1..=MOST_ITEMS, DEFAULT_ITEMS)?;
    let offset = offset_of(args.cursor)?;
    let graph = FalkorGraph::connect(config).await?;
    stored_concept(&graph, id).await?;
    if let Some(document) = document {
        stored_title(&graph, document).await?;
    }
    // One more than the limit is read, to know whether there is a next page.
    let mut items = graph
        .items_mentioning(id, document, kind, offset, limit + 1)
        .await?;
    let next_cursor = (items.len() > limit).then(|| (offset + limit).to_string());
    items.truncate(limit);
    Ok(ConceptItemsResult {
        concept_id: id.to_string(),
        items: items.into_iter().map(ItemRow::from).collect(),
        next_cursor,
    })
}

/// The stored items nearest to the vector that is stored for an item. Nothing is embedded.
///
/// # Errors
/// A bad argument, an item that is not stored, or an item store that is not ready.
pub(crate) async fn similar_items(
    config: &Config,
    args: SimilarItemsArgs,
) -> Result<SimilarItemsResult, ConceptsError> {
    let id = args
        .item_id
        .trim()
        .parse()
        .map_err(ConceptsError::NotAnItemId)?;
    let kind = parse_item_kind(args.kind)?;
    let limit = bounded("limit", args.limit, 1..=MOST_SIMILAR, DEFAULT_SIMILAR)?;
    let store = ItemStore::connect(config).map_err(ConceptsError::ItemStore)?;
    let filter = ItemFilter {
        kind,
        documents: None,
        items: None,
    };
    let hits = store
        .search_by_point(id, &filter, limit)
        .await
        .map_err(ConceptsError::ItemStore)?
        .ok_or(ConceptsError::UnknownItem { id })?;
    Ok(SimilarItemsResult {
        item_id: id.to_string(),
        items: hits.into_iter().map(SimilarItem::from).collect(),
    })
}
