//! What the item store and the concept store share: the Qdrant client, the one shape that every
//! collection has, the id of a point as text, and the errors of both stores.

use std::time::Duration;

use qdrant_client::qdrant::{
    CreateCollectionBuilder, Distance, PointId, VectorParamsBuilder, point_id::PointIdOptions,
};
use qdrant_client::{Qdrant, QdrantError};

use crate::{ConceptId, Config, EMBEDDING_DIMENSIONS, ItemId};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

// SMELL: this type serves both stores, but `Payload`, `PayloadShape`, `PointId` and
// `StoredPayload` are errors of the item store only, and their names do not say so.
#[derive(thiserror::Error, Debug)]
pub enum StoreError {
    // The Qdrant errors are boxed because they are large and every result would carry that size.
    #[error("could not set up the Qdrant client for {url}")]
    Connect {
        url: String,
        #[source]
        source: Box<QdrantError>,
    },

    #[error("could not reach Qdrant at {url}")]
    Unreachable {
        url: String,
        #[source]
        source: Box<QdrantError>,
    },

    #[error("Qdrant at {url} failed to {action} in the collection {collection}")]
    Request {
        url: String,
        collection: String,
        action: &'static str,
        #[source]
        source: Box<QdrantError>,
    },

    #[error("the payload of item {id} could not be written as json")]
    Payload {
        id: ItemId,
        #[source]
        source: serde_json::Error,
    },

    #[error("the payload of item {id} is not a json object")]
    PayloadShape { id: ItemId },

    #[error("a point of the collection {collection} has the id {point}, which is not an item id")]
    PointId {
        collection: String,
        point: String,
        #[source]
        source: uuid::Error,
    },

    #[error(
        "the payload stored for item {id} in the collection {collection} is not the payload of an item"
    )]
    StoredPayload {
        id: ItemId,
        collection: String,
        #[source]
        source: serde_json::Error,
    },

    #[error(
        "a point of the collection {collection} has the id {point}, which is not a concept id; QDRANT_CONCEPTS_COLLECTION must name a collection that holds concepts"
    )]
    ConceptPointId {
        collection: String,
        point: String,
        #[source]
        source: uuid::Error,
    },

    #[error(
        "the payload stored for concept {id} in the collection {collection} is not the payload of a concept; QDRANT_CONCEPTS_COLLECTION must name a collection that holds concepts"
    )]
    StoredConceptPayload {
        id: ConceptId,
        collection: String,
        #[source]
        source: serde_json::Error,
    },
}

/// The client for the Qdrant address in the config. It makes no network call.
pub(crate) fn build_client(config: &Config) -> Result<Qdrant, StoreError> {
    // Without `skip_compatibility_check` the client starts a thread on build, and it prints to
    // standard output when the store is down, which would end up in the middle of a report.
    Qdrant::from_url(&config.qdrant_url)
        .timeout(REQUEST_TIMEOUT)
        .skip_compatibility_check()
        .build()
        .map_err(|source| StoreError::Connect {
            url: config.qdrant_url.clone(),
            source: Box::new(source),
        })
}

// SMELL: each store holds a client, an address and a collection name, and passes them one by one:
// the address and the name to this function, and all three to `ensure_collection`. The address
// and the name are both text, so a call that swaps them compiles.
pub(crate) fn request_error(
    url: &str,
    collection: &str,
    action: &'static str,
    source: QdrantError,
) -> StoreError {
    StoreError::Request {
        url: url.to_owned(),
        collection: collection.to_owned(),
        action,
        source: Box::new(source),
    }
}

pub(crate) async fn ensure_collection(
    client: &Qdrant,
    url: &str,
    collection: &str,
) -> Result<(), StoreError> {
    // SMELL: looking and creating are two calls, so two programs that start together can both
    // try to create the collection, and the second one fails.
    let exists = client
        .collection_exists(collection)
        .await
        .map_err(|source| request_error(url, collection, "look for the collection", source))?;
    if exists {
        return Ok(());
    }
    let vectors = VectorParamsBuilder::new(EMBEDDING_DIMENSIONS as u64, Distance::Cosine);
    client
        .create_collection(CreateCollectionBuilder::new(collection).vectors_config(vectors))
        .await
        .map_err(|source| request_error(url, collection, "create the collection", source))?;
    Ok(())
}

/// The id of a point as text. A point with no id gives an empty text, which is not a valid id.
pub(crate) fn point_id_text(id: Option<PointId>) -> String {
    match id.and_then(|id| id.point_id_options) {
        Some(PointIdOptions::Uuid(text)) => text,
        Some(PointIdOptions::Num(number)) => number.to_string(),
        None => String::new(),
    }
}
