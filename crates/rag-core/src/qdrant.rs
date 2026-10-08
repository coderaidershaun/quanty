//! What the item store and the concept store share: one collection of Qdrant with its client, the
//! one shape that every collection has, the id of a point as text, and the errors of both stores.

use std::time::Duration;

use qdrant_client::qdrant::{
    CreateCollectionBuilder, Distance, PointId, VectorParamsBuilder, point_id::PointIdOptions,
};
use qdrant_client::{Qdrant, QdrantError};

use crate::{ConceptId, Config, EMBEDDING_DIMENSIONS, ItemId};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

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
    ItemPayload {
        id: ItemId,
        #[source]
        source: serde_json::Error,
    },

    #[error("the payload of item {id} is not a json object")]
    ItemPayloadShape { id: ItemId },

    #[error("a point of the collection {collection} has the id {point}, which is not an item id")]
    ItemPointId {
        collection: String,
        point: String,
        #[source]
        source: uuid::Error,
    },

    #[error(
        "the payload stored for item {id} in the collection {collection} is not the payload of an item"
    )]
    StoredItemPayload {
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

pub(crate) struct Collection {
    pub(crate) client: Qdrant,
    pub(crate) url: String,
    pub(crate) name: String,
}

impl Collection {
    /// It makes no network call.
    pub(crate) fn open(config: &Config, name: &str) -> Result<Collection, StoreError> {
        // Without `skip_compatibility_check` the client starts a thread on build, and it prints to
        // standard output when the store is down, which would end up in the middle of a report.
        let client = Qdrant::from_url(&config.qdrant_url)
            .timeout(REQUEST_TIMEOUT)
            .skip_compatibility_check()
            .build()
            .map_err(|source| StoreError::Connect {
                url: config.qdrant_url.clone(),
                source: Box::new(source),
            })?;
        Ok(Collection {
            client,
            url: config.qdrant_url.clone(),
            name: name.to_owned(),
        })
    }

    pub(crate) fn request_error(&self, action: &'static str, source: QdrantError) -> StoreError {
        StoreError::Request {
            url: self.url.clone(),
            collection: self.name.clone(),
            action,
            source: Box::new(source),
        }
    }

    pub(crate) async fn exists(&self) -> Result<bool, StoreError> {
        self.client
            .collection_exists(self.name.as_str())
            .await
            .map_err(|source| self.request_error("look for the collection", source))
    }

    pub(crate) async fn ensure(&self) -> Result<(), StoreError> {
        if self.exists().await? {
            return Ok(());
        }
        let vectors = VectorParamsBuilder::new(EMBEDDING_DIMENSIONS as u64, Distance::Cosine);
        let created = self
            .client
            .create_collection(
                CreateCollectionBuilder::new(self.name.as_str()).vectors_config(vectors),
            )
            .await;
        match created {
            Ok(_) => Ok(()),
            // Another program can create the collection after the look above, and Qdrant then
            // refuses this request. The collection is there, which is all that was asked for.
            Err(source) => match self.exists().await {
                Ok(true) => Ok(()),
                _ => Err(self.request_error("create the collection", source)),
            },
        }
    }
}

/// A point with no id gives an empty text, which is not a valid id.
pub(crate) fn point_id_text(id: Option<PointId>) -> String {
    match id.and_then(|id| id.point_id_options) {
        Some(PointIdOptions::Uuid(text)) => text,
        Some(PointIdOptions::Num(number)) => number.to_string(),
        None => String::new(),
    }
}
