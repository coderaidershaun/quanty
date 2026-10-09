//! The Qdrant collection that holds every item: one unnamed vector and the payload of an item
//! for each point. Ingestion writes it and retrieval reads it, so both use this one definition.

mod points;
mod search;

use qdrant_client::qdrant::{CreateFieldIndexCollectionBuilder, FieldType};

pub use search::{ItemFilter, ItemHit};

use crate::qdrant::Collection;
use crate::{Config, Embedding, ItemId, ItemPayload, StoreError};

#[derive(Debug, Clone, PartialEq)]
pub struct ItemPoint {
    pub id: ItemId,
    pub vector: Embedding,
    pub payload: ItemPayload,
}

pub struct ItemStore {
    collection: Collection,
}

impl ItemStore {
    /// Makes no network call, so it works while the store is down. The first call that needs the
    /// store is where a failure shows.
    ///
    /// # Errors
    /// [`StoreError::Connect`] when the address in the config is not a valid URL.
    pub fn connect(config: &Config) -> Result<ItemStore, StoreError> {
        Ok(ItemStore {
            collection: Collection::open(config, &config.items_collection)?,
        })
    }

    pub fn collection(&self) -> &str {
        &self.collection.name
    }

    /// # Errors
    /// [`StoreError::Unreachable`] when Qdrant does not answer a health check.
    pub async fn ping(&self) -> Result<(), StoreError> {
        self.collection
            .client
            .health_check()
            .await
            .map(|_| ())
            .map_err(|source| StoreError::Unreachable {
                url: self.collection.url.clone(),
                source: Box::new(source),
            })
    }

    /// Creates the collection when it is missing: one unnamed vector of the embedder's length for
    /// each point, compared by cosine distance. An existing collection keeps its points and its
    /// shape, which is not checked. A collection that has no index on the kind of its items gets
    /// one, so that a search by kind does not read the kind of every point it visits.
    ///
    /// # Errors
    /// [`StoreError::Request`] when Qdrant refuses or cannot be reached.
    pub async fn ensure_collection(&self) -> Result<(), StoreError> {
        self.collection.ensure().await?;
        let index = CreateFieldIndexCollectionBuilder::new(
            self.collection.name.as_str(),
            KIND_FIELD,
            FieldType::Keyword,
        )
        .wait(true);
        self.collection
            .client
            .create_field_index(index)
            .await
            .map_err(|source| {
                self.collection
                    .request_error("index the kind of the items", source)
            })?;
        Ok(())
    }

    /// Whether the collection is there. It never creates it.
    ///
    /// # Errors
    /// [`StoreError::Request`] when Qdrant refuses or cannot be reached.
    pub async fn collection_exists(&self) -> Result<bool, StoreError> {
        self.collection.exists().await
    }
}

/// The name of the payload field that holds an item's kind. It must match the field name of
/// `ItemPayload`.
const KIND_FIELD: &str = "kind";

/// The name of the payload field that holds the document of an item. It must match the field name
/// of `ItemPayload`.
const DOC_ID_FIELD: &str = "doc_id";
