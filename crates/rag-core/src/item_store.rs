//! The Qdrant collection that holds every item: one unnamed vector and the payload of an item
//! for each point. Ingestion writes it and retrieval reads it, so both use this one definition
//! of the collection.

use qdrant_client::qdrant::{
    Condition, CountPointsBuilder, DeletePointsBuilder, Filter, PointStruct, QueryPointsBuilder,
    ScoredPoint, UpsertPointsBuilder,
};
use qdrant_client::{Payload, Qdrant, QdrantError};

use crate::qdrant::{build_client, ensure_collection, point_id_text, request_error};
use crate::{Config, DocId, Embedding, ItemId, ItemKind, ItemPayload, StoreError};

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

impl ItemStore {
    /// Makes no network call, so it works while the store is down. The first call that needs the
    /// store is where a failure shows.
    ///
    /// # Errors
    /// [`StoreError::Connect`] when the address in the config is not a valid URL.
    pub fn connect(config: &Config) -> Result<ItemStore, StoreError> {
        Ok(ItemStore {
            client: build_client(config)?,
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
        ensure_collection(&self.client, &self.url, &self.collection).await
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

    /// Removes every point of one document and returns how many points that was. A document
    /// with no points gives `0`, which is not an error.
    ///
    /// # Errors
    /// [`StoreError::Request`] when Qdrant refuses or cannot be reached. A collection that does
    /// not exist is not a special case: the count fails, and that error, which names the
    /// collection, is passed on.
    pub async fn delete_document(&self, document: DocId) -> Result<u64, StoreError> {
        let of_document = Filter::must([Condition::matches("doc_id", document.to_string())]);
        // SMELL: counting and deleting are two calls, so points of the document that another
        // program stores between the two are removed but not counted, and the number returned
        // is then too low.
        let reply = self
            .client
            .count(
                CountPointsBuilder::new(self.collection.as_str())
                    .filter(of_document.clone())
                    .exact(true),
            )
            .await
            .map_err(|source| self.request_error("count the points of the document", source))?;
        self.client
            .delete_points(
                DeletePointsBuilder::new(self.collection.as_str())
                    .points(of_document)
                    .wait(true),
            )
            .await
            .map_err(|source| self.request_error("delete the points of the document", source))?;
        Ok(reply.result.map_or(0, |result| result.count))
    }

    fn request_error(&self, action: &'static str, source: QdrantError) -> StoreError {
        request_error(&self.url, &self.collection, action, source)
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
    let id_text = point_id_text(point.id);
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
