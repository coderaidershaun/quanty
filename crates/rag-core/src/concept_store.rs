//! The Qdrant collection that holds one point for each concept. Ingestion writes it and
//! retrieval searches it, so both use this one definition of the collection.

use qdrant_client::qdrant::{
    PointStruct, PointsIdsList, QueryPointsBuilder, ScoredPoint, SetPayloadPointsBuilder,
    UpsertPointsBuilder,
};
use qdrant_client::{Payload, Qdrant, QdrantError};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::qdrant::{build_client, ensure_collection, point_id_text, request_error};
use crate::{ConceptId, Config, DocumentInput, Embedding, StoreError};

const NAME_FIELD: &str = "name";
const ALIASES_FIELD: &str = "aliases";

/// What is embedded for a concept: its name and its one-line definition, as "name: definition".
pub fn concept_input(name: &str, definition: &str) -> DocumentInput {
    DocumentInput {
        title: String::new(),
        text: format!("{name}: {definition}"),
        image: None,
    }
}

/// A concept ready to be stored.
#[derive(Debug, Clone, PartialEq)]
pub struct ConceptPoint {
    pub id: ConceptId,
    pub vector: Embedding,
    pub name: String,
    pub aliases: Vec<String>,
}

/// A stored concept that a search found.
#[derive(Debug, Clone, PartialEq)]
pub struct ConceptHit {
    pub id: ConceptId,
    /// The cosine similarity to the vector that was searched for: higher is nearer.
    pub score: f32,
    pub name: String,
    pub aliases: Vec<String>,
}

/// Reads and writes the one collection of concepts that the config names. Its payload holds only
/// the name and the aliases: the definition lives in the graph.
pub struct ConceptStore {
    client: Qdrant,
    url: String,
    collection: String,
}

impl ConceptStore {
    /// Makes no network call, so it works while the store is down. The first call that needs the
    /// store is where a failure shows.
    ///
    /// # Errors
    /// [`StoreError::Connect`] when the address in the config is not a valid URL.
    pub fn connect(config: &Config) -> Result<ConceptStore, StoreError> {
        Ok(ConceptStore {
            client: build_client(config)?,
            url: config.qdrant_url.clone(),
            collection: config.concepts_collection.clone(),
        })
    }

    pub fn collection(&self) -> &str {
        &self.collection
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

    /// Stores the concept under its own id, replacing the point that has that id, and returns once
    /// Qdrant has applied it.
    ///
    /// # Errors
    /// [`StoreError::Request`] when Qdrant refuses or cannot be reached.
    pub async fn upsert(&self, concept: &ConceptPoint) -> Result<(), StoreError> {
        let point = PointStruct::new(
            concept.id.to_string(),
            concept.vector.clone(),
            concept_payload(&concept.name, &concept.aliases),
        );
        self.client
            .upsert_points(
                UpsertPointsBuilder::new(self.collection.as_str(), vec![point]).wait(true),
            )
            .await
            .map_err(|source| self.request_error("store the concept", source))?;
        Ok(())
    }

    /// Replaces the aliases of one point, leaves its name and its vector as they are, and returns
    /// once Qdrant has applied it. A point that is not there is not created.
    ///
    /// # Errors
    /// [`StoreError::Request`] when Qdrant refuses or cannot be reached.
    pub async fn set_aliases(&self, id: ConceptId, aliases: &[String]) -> Result<(), StoreError> {
        let fields = Map::from_iter([(ALIASES_FIELD.to_owned(), Value::from(aliases))]);
        self.client
            .set_payload(
                SetPayloadPointsBuilder::new(self.collection.as_str(), Payload::from(fields))
                    .points_selector(PointsIdsList {
                        ids: vec![id.to_string().into()],
                    })
                    .wait(true),
            )
            .await
            .map_err(|source| self.request_error("set the aliases of the concept", source))?;
        Ok(())
    }

    /// The `limit` concepts whose vectors are nearest to `vector`, nearest first. It never
    /// creates the collection.
    ///
    /// # Errors
    /// - [`StoreError::Request`] when Qdrant refuses, cannot be reached, or has no such collection
    /// - [`StoreError::ConceptPointId`] and [`StoreError::StoredConceptPayload`] when a stored
    ///   point is not a concept; one such point fails the whole search
    pub async fn nearest(
        &self,
        vector: Embedding,
        limit: usize,
    ) -> Result<Vec<ConceptHit>, StoreError> {
        let query = QueryPointsBuilder::new(self.collection.as_str())
            .query(vector)
            .limit(limit as u64)
            .with_payload(true);
        let reply = self
            .client
            .query(query)
            .await
            .map_err(|source| self.request_error("search for concepts", source))?;
        reply
            .result
            .into_iter()
            .map(|point| concept_hit(&self.collection, point))
            .collect()
    }

    fn request_error(&self, action: &'static str, source: QdrantError) -> StoreError {
        request_error(&self.url, &self.collection, action, source)
    }
}

fn concept_payload(name: &str, aliases: &[String]) -> Payload {
    Payload::from(Map::from_iter([
        (NAME_FIELD.to_owned(), Value::from(name)),
        (ALIASES_FIELD.to_owned(), Value::from(aliases)),
    ]))
}

// SMELL: the field names of this struct repeat `NAME_FIELD` and `ALIASES_FIELD`, and nothing keeps
// them the same. If one side is renamed alone, no stored concept can be read.
#[derive(Deserialize)]
struct StoredConcept {
    name: String,
    aliases: Vec<String>,
}

fn concept_hit(collection: &str, point: ScoredPoint) -> Result<ConceptHit, StoreError> {
    let id_text = point_id_text(point.id);
    let id = id_text
        .parse::<ConceptId>()
        .map_err(|source| StoreError::ConceptPointId {
            collection: collection.to_owned(),
            point: id_text,
            source,
        })?;
    let json = Value::from(Payload::from(point.payload));
    let stored = serde_json::from_value::<StoredConcept>(json).map_err(|source| {
        StoreError::StoredConceptPayload {
            id,
            collection: collection.to_owned(),
            source,
        }
    })?;
    Ok(ConceptHit {
        id,
        score: point.score,
        name: stored.name,
        aliases: stored.aliases,
    })
}
