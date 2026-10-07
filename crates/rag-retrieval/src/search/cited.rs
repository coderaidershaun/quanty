//! The last step of a search: the figures, tables and equations that a shown paragraph cites by
//! their printed label.

use rag_core::{Embedding, ItemStore};

use super::SearchError;
use super::results::{Reason, SearchHit};

/// Adds, after the results, each item of the same document that a result cites and that is not
/// in the results yet. The added items do not count toward the number of results, and they are
/// not ranked with the others.
///
/// # Errors
/// [`SearchError::Items`] when the items cannot be searched.
pub(super) async fn pull_in(
    items: &ItemStore,
    vector: &Embedding,
    hits: &mut Vec<SearchHit>,
) -> Result<(), SearchError> {
    for index in 0..hits.len() {
        let payload = &hits[index].item.payload;
        let (document, labels) = (payload.doc_id, payload.cites.clone());
        let cited = items
            .labelled(vector.clone(), document, &labels)
            .await
            .map_err(SearchError::Items)?;
        for item in cited {
            let already_there = hits.iter().any(|hit| hit.item.id == item.id);
            let Some(label) = item.payload.label.clone() else {
                continue;
            };
            if !already_there {
                let reason = Reason::Cited {
                    by: index + 1,
                    label,
                };
                hits.push(SearchHit { item, reason });
            }
        }
    }
    Ok(())
}
