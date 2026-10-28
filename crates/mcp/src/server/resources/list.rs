//! Lists the media and the documents of the library as resources.

use rag_core::Config;
use rmcp::ErrorData;
use rmcp::model::{CacheScope, ListResourcesResult, Resource};
use serde_json::json;

use super::error::ResourceError;
use super::read::args;
use super::uri::{CHANGING_TTL_MS, MIME_TYPE, document_uri, media_uri};
use crate::library;

/// The most documents `list_documents` gives in one page.
const LARGEST_PAGE: u32 = 500;

/// One resource for each media and one for each document, all in one page.
///
/// # Errors
/// An internal error when the graph cannot be read.
pub(crate) async fn list(
    config: &Config,
    sends_cache_hints: bool,
) -> Result<ListResourcesResult, ErrorData> {
    let resources = resources(config).await.map_err(ResourceError::internal)?;
    let listed = ListResourcesResult::with_all_items(resources);
    Ok(if sends_cache_hints {
        listed
            .with_ttl_ms(CHANGING_TTL_MS)
            .with_cache_scope(CacheScope::Private)
    } else {
        listed
    })
}

async fn resources(config: &Config) -> Result<Vec<Resource>, ResourceError> {
    let media = library::list_media(config, args(json!({ "detail": "names" }))?)
        .await
        .map_err(ResourceError::from_reader)?;
    let mut resources: Vec<Resource> = media
        .titles()
        .map(|title| Resource::new(media_uri(title), title).with_mime_type(MIME_TYPE))
        .collect();
    let mut cursor = None;
    loop {
        let page = args(json!({ "detail": "names", "limit": LARGEST_PAGE, "cursor": cursor }))?;
        let documents = library::list_documents(config, page)
            .await
            .map_err(ResourceError::from_reader)?;
        resources.extend(documents.names().map(|(document_id, title)| {
            Resource::new(document_uri(document_id), title).with_mime_type(MIME_TYPE)
        }));
        cursor = documents.cursor().map(str::to_owned);
        if cursor.is_none() {
            return Ok(resources);
        }
    }
}
