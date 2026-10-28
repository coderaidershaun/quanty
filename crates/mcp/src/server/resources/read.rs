//! Reads a resource by calling the reader that the matching tool calls, with the arguments that
//! tool would get from an agent that left everything but the address out. A resource therefore
//! gives exactly what that tool gives: the same view, with the same defaults.

use rag_core::Config;
use rmcp::ErrorData;
use rmcp::model::{CacheScope, ReadResourceResult, ResourceContents};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::error::{ResourceError, no_such_resource};
use super::uri::{MIME_TYPE, ResourceUri};
use crate::concepts::{self, ConceptArgs};
use crate::library::{self, LibraryError};
use crate::server::failure::ToolError;

/// What `resources/read` answers for `uri`. A peer that understands cache hints is told how long
/// the view stays fresh.
///
/// # Errors
/// A missing resource when `uri` names nothing stored, and an internal error when a store is down
/// or a chapter cannot be read.
pub(crate) async fn read(
    config: &Config,
    uri: &str,
    sends_cache_hints: bool,
) -> Result<ReadResourceResult, ErrorData> {
    let resource = ResourceUri::parse(uri).ok_or_else(|| no_such_resource(uri))?;
    let text = view_text(&resource, config)
        .await
        .map_err(|error| error.for_uri(uri))?;
    let result = ReadResourceResult::new(vec![
        ResourceContents::text(text, uri).with_mime_type(MIME_TYPE),
    ]);
    Ok(if sends_cache_hints {
        result
            .with_ttl_ms(resource.ttl_ms())
            .with_cache_scope(CacheScope::Private)
    } else {
        result
    })
}

/// The view that the matching tool gives, as the JSON text of its result.
async fn view_text(resource: &ResourceUri, config: &Config) -> Result<String, ResourceError> {
    match resource {
        ResourceUri::Media { title } => {
            let args = args(json!({ "media": title }))?;
            let documents = library::list_documents(config, args)
                .await
                .map_err(ResourceError::from_reader)?;
            if documents.total() == 0 {
                return Err(ResourceError::from_reader(
                    LibraryError::NoDocumentOfMedia {
                        title: title.clone(),
                    },
                ));
            }
            text_of(&documents)
        }
        ResourceUri::Overview { document_id } => {
            let args = args(json!({ "document_id": document_id }))?;
            text_of_result(library::document_overview(config, args).await)
        }
        ResourceUri::Outline { document_id } => {
            let args = args(json!({ "document_id": document_id }))?;
            text_of_result(library::document_outline(config, args).await)
        }
        ResourceUri::Section { document_id, index } => {
            let args = args(json!({ "document_id": document_id, "section": index }))?;
            text_of_result(library::read_section(config, args))
        }
        ResourceUri::Page { document_id, page } => {
            let args = args(json!({ "document_id": document_id, "page": page }))?;
            text_of_result(library::read_page(config, args))
        }
        ResourceUri::Insights { document_id } => {
            let args = args(json!({ "document_id": document_id }))?;
            text_of_result(library::chapter_insights(config, args).await)
        }
        ResourceUri::Concept { concept_id } => {
            let args: ConceptArgs = args(json!({ "concept_id": concept_id }))?;
            let reference = args.reference().map_err(ResourceError::from_reader)?;
            text_of_result(concepts::concept(config, reference).await)
        }
    }
}

/// The arguments of a reader, read from JSON as an agent would send them.
pub(super) fn args<A: DeserializeOwned>(given: Value) -> Result<A, ResourceError> {
    serde_json::from_value(given).map_err(ResourceError::Arguments)
}

fn text_of_result<V: Serialize, E: Into<ToolError>>(
    result: Result<V, E>,
) -> Result<String, ResourceError> {
    text_of(&result.map_err(ResourceError::from_reader)?)
}

fn text_of(view: &impl Serialize) -> Result<String, ResourceError> {
    serde_json::to_string(view).map_err(ResourceError::Json)
}
