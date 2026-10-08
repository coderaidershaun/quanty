//! The fourth and fifth steps of a search: one query that puts the seeds and the items that the
//! graph added on the same scale, and the cap on how many results one document gives.

use std::collections::{BTreeMap, HashMap};

use rag_core::{DocId, Embedding, ItemFilter, ItemHit, ItemId, ItemStore};

use super::results::{Reason, SearchHit};
use super::{MAX_RESULTS_PER_DOCUMENT, RESULTS_PER_QUERY, SearchError};

pub(super) struct Ranking {
    /// How many of the candidates the item store gave back.
    pub(super) ranked: usize,
    pub(super) kept: Vec<SearchHit>,
    /// The items passed over because their document already had [`MAX_RESULTS_PER_DOCUMENT`]
    /// results, nearest first.
    pub(super) capped: Vec<ItemHit>,
}

/// # Errors
/// [`SearchError::Items`] when the items cannot be searched.
pub(super) async fn ranked_within_the_cap(
    items: &ItemStore,
    vector: &Embedding,
    filter: &ItemFilter,
    candidates: BTreeMap<ItemId, Reason>,
) -> Result<Ranking, SearchError> {
    let ids: Vec<ItemId> = candidates.keys().copied().collect();
    let ranked = items
        .rank(vector.clone(), &ids, filter)
        .await
        .map_err(SearchError::Items)?;
    Ok(keep_within_the_cap(ranked, candidates))
}

fn keep_within_the_cap(ranked: Vec<ItemHit>, mut candidates: BTreeMap<ItemId, Reason>) -> Ranking {
    let ranked_count = ranked.len();
    let mut kept_of_document: HashMap<DocId, usize> = HashMap::new();
    let mut kept = Vec::new();
    let mut capped = Vec::new();
    for item in ranked {
        if kept.len() == RESULTS_PER_QUERY {
            break;
        }
        let in_document = kept_of_document.entry(item.payload.doc_id).or_insert(0);
        if *in_document == MAX_RESULTS_PER_DOCUMENT {
            capped.push(item);
            continue;
        }
        let Some(reason) = candidates.remove(&item.id) else {
            continue;
        };
        *in_document += 1;
        kept.push(SearchHit { item, reason });
    }
    Ranking {
        ranked: ranked_count,
        kept,
        capped,
    }
}
