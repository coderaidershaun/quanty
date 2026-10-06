//! The Qdrant collection that holds every item: one unnamed vector and the payload of an item
//! for each point. Ingestion writes it and retrieval reads it, so both use this one definition
//! of the collection.

use std::time::Duration;

use qdrant_client::qdrant::{
    Condition, CountPointsBuilder, CreateCollectionBuilder, Distance, Filter, PointStruct,
    QueryPointsBuilder, ScoredPoint, UpsertPointsBuilder, VectorParamsBuilder,
    point_id::PointIdOptions,
};
use qdrant_client::{Payload, Qdrant, QdrantError};

use crate::{Config, EMBEDDING_DIMENSIONS, Embedding, ItemId, ItemKind, ItemPayload};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// A group of this many points with their vectors is about a megabyte, well inside a request.
const UPSERT_GROUP_SIZE: usize = 256;

/// An item ready to be stored.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemPoint {
    pub id: ItemId,
    pub vector: Embedding,
    pub payload: ItemPayload,
}

/// Reads and writes the one collection that the config names.
pub struct ItemStore {
    client: Qdrant,
    url: String,
    collection: String,
}

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
}

impl ItemStore {
    /// Makes no network call, so it works while the store is down. The first call that needs the
    /// store is where a failure shows.
    ///
    /// # Errors
    /// [`StoreError::Connect`] when the address in the config is not a valid URL.
    pub fn connect(config: &Config) -> Result<ItemStore, StoreError> {
        // Without this the client starts a thread on build, and it prints to standard output when
        // the store is down, which would end up in the middle of a report.
        let client = Qdrant::from_url(&config.qdrant_url)
            .timeout(REQUEST_TIMEOUT)
            .skip_compatibility_check()
            .build()
            .map_err(|source| StoreError::Connect {
                url: config.qdrant_url.clone(),
                source: Box::new(source),
            })?;
        Ok(ItemStore {
            client,
            url: config.qdrant_url.clone(),
            collection: config.items_collection.clone(),
        })
    }

    pub fn collection(&self) -> &str {
        &self.collection
    }

    /// # Errors
    /// [`StoreError::Unreachable`] when Qdrant does not answer a health check.
    pub async fn ping(&self) -> Result<(), StoreError> {
        self.client
            .health_check()
            .await
            .map(|_| ())
            .map_err(|source| StoreError::Unreachable {
                url: self.url.clone(),
                source: Box::new(source),
            })
    }

    /// Creates the collection when it is missing: one unnamed vector of the embedder's length for
    /// each point, compared by cosine distance. An existing collection is left as it is, and is
    /// not checked to have that shape.
    ///
    /// # Errors
    /// [`StoreError::Request`] when Qdrant refuses or cannot be reached.
    pub async fn ensure_collection(&self) -> Result<(), StoreError> {
        // SMELL: looking and creating are two calls, so two programs that start together can
        // both try to create the collection, and the second one fails.
        let exists = self
            .client
            .collection_exists(self.collection.as_str())
            .await
            .map_err(|source| self.request_error("look for the collection", source))?;
        if exists {
            return Ok(());
        }
        let vectors = VectorParamsBuilder::new(EMBEDDING_DIMENSIONS as u64, Distance::Cosine);
        self.client
            .create_collection(
                CreateCollectionBuilder::new(self.collection.as_str()).vectors_config(vectors),
            )
            .await
            .map_err(|source| self.request_error("create the collection", source))?;
        Ok(())
    }

    /// Stores the points, replacing any point that has the same identifier, and returns once
    /// Qdrant has applied them. An empty slice makes no call.
    ///
    /// # Errors
    /// [`StoreError::Payload`] or [`StoreError::PayloadShape`] for a payload that cannot be
    /// sent, [`StoreError::Request`] when Qdrant refuses or cannot be reached.
    pub async fn upsert(&self, points: &[ItemPoint]) -> Result<(), StoreError> {
        for group in points.chunks(UPSERT_GROUP_SIZE) {
            let structs = group
                .iter()
                .map(point_struct)
                .collect::<Result<Vec<_>, _>>()?;
            self.client
                .upsert_points(
                    UpsertPointsBuilder::new(self.collection.as_str(), structs).wait(true),
                )
                .await
                .map_err(|source| self.request_error("store points", source))?;
        }
        Ok(())
    }

    /// The exact number of points in the collection.
    ///
    /// # Errors
    /// [`StoreError::Request`] when Qdrant refuses or cannot be reached.
    pub async fn count(&self) -> Result<u64, StoreError> {
        let reply = self
            .client
            .count(CountPointsBuilder::new(self.collection.as_str()).exact(true))
            .await
            .map_err(|source| self.request_error("count points", source))?;
        Ok(reply.result.map_or(0, |result| result.count))
    }

    fn request_error(&self, action: &'static str, source: QdrantError) -> StoreError {
        StoreError::Request {
            url: self.url.clone(),
            collection: self.collection.clone(),
            action,
            source: Box::new(source),
        }
    }
}

fn point_struct(point: &ItemPoint) -> Result<PointStruct, StoreError> {
    let json = serde_json::to_value(&point.payload).map_err(|source| StoreError::Payload {
        id: point.id,
        source,
    })?;
    let payload = Payload::try_from(json).map_err(|_| StoreError::PayloadShape { id: point.id })?;
    Ok(PointStruct::new(
        point.id.to_string(),
        point.vector.clone(),
        payload,
    ))
}

/// A stored item that a search found.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemHit {
    pub id: ItemId,
    /// The cosine similarity to the question: higher is nearer.
    pub score: f32,
    pub payload: ItemPayload,
}

/// The name of the payload field that holds an item's kind. It must match the field name of
/// `ItemPayload`.
const KIND_FIELD: &str = "kind";

impl ItemStore {
    /// The `limit` items whose vectors are nearest to `vector`, nearest first, among the items of
    /// `kind` when one is given. It never creates the collection.
    ///
    /// # Errors
    /// - [`StoreError::Request`] when Qdrant refuses, cannot be reached, or has no such collection
    /// - [`StoreError::PointId`] and [`StoreError::StoredPayload`] when a stored point is not an
    ///   item; one such point fails the whole search
    pub async fn search(
        &self,
        vector: Embedding,
        kind: Option<ItemKind>,
        limit: usize,
    ) -> Result<Vec<ItemHit>, StoreError> {
        let mut query = QueryPointsBuilder::new(self.collection.as_str())
            .query(vector)
            .limit(limit as u64)
            .with_payload(true);
        // SMELL: the collection has no index on the kind field, so Qdrant reads the kind of each
        // point it visits. That is fine for a few chapters and slow for a large collection.
        if let Some(kind) = kind {
            query = query.filter(Filter::must([Condition::matches(
                KIND_FIELD,
                kind.as_str().to_owned(),
            )]));
        }
        let reply = self
            .client
            .query(query)
            .await
            .map_err(|source| self.request_error("search for items", source))?;
        reply
            .result
            .into_iter()
            .map(|point| item_hit(&self.collection, point))
            .collect()
    }
}

fn item_hit(collection: &str, point: ScoredPoint) -> Result<ItemHit, StoreError> {
    let id_text = match point.id.and_then(|id| id.point_id_options) {
        Some(PointIdOptions::Uuid(text)) => text,
        Some(PointIdOptions::Num(number)) => number.to_string(),
        None => String::new(),
    };
    let id = id_text
        .parse::<ItemId>()
        .map_err(|source| StoreError::PointId {
            collection: collection.to_owned(),
            point: id_text,
            source,
        })?;
    let json = serde_json::Value::from(Payload::from(point.payload));
    let payload = serde_json::from_value::<ItemPayload>(json).map_err(|source| {
        StoreError::StoredPayload {
            id,
            collection: collection.to_owned(),
            source,
        }
    })?;
    Ok(ItemHit {
        id,
        score: point.score,
        payload,
    })
}
