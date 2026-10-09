//! Writes the points of the items: stores them, counts them, deletes the points of a document and
//! writes the labels of a document onto every point of it.

use std::collections::BTreeSet;

use qdrant_client::Payload;
use qdrant_client::qdrant::{
    Condition, CountPointsBuilder, DeletePayloadPointsBuilder, DeletePointsBuilder, Filter,
    PointStruct, SetPayloadPointsBuilder, UpsertPointsBuilder,
};
use serde_json::{Map, Value};

use super::{DOC_ID_FIELD, ItemPoint, ItemStore};
use crate::{DocId, DocumentLabels, StoreError, Tag};

/// A group of this many points with their vectors is about a megabyte, well inside a request.
const UPSERT_GROUP_SIZE: usize = 256;

/// The names of the payload fields that hold the labels of a document. They must match the field
/// names of `DocumentLabels`.
const MEDIA_FIELD: &str = "media";
const CATEGORY_FIELD: &str = "category";
const AUTHORS_FIELD: &str = "authors";
const MEDIA_TAGS_FIELD: &str = "media_tags";
const TAGS_FIELD: &str = "tags";

impl ItemStore {
    /// Stores the points, replacing any point that has the same identifier, and returns once
    /// Qdrant has applied them. An empty slice makes no call.
    ///
    /// # Errors
    /// [`StoreError::ItemPayload`] or [`StoreError::ItemPayloadShape`] for a payload that cannot be
    /// sent, [`StoreError::Request`] when Qdrant refuses or cannot be reached.
    pub async fn upsert(&self, points: &[ItemPoint]) -> Result<(), StoreError> {
        for group in points.chunks(UPSERT_GROUP_SIZE) {
            let structs = group
                .iter()
                .map(point_struct)
                .collect::<Result<Vec<_>, _>>()?;
            self.collection
                .client
                .upsert_points(
                    UpsertPointsBuilder::new(self.collection.name.as_str(), structs).wait(true),
                )
                .await
                .map_err(|source| self.collection.request_error("store points", source))?;
        }
        Ok(())
    }

    /// The exact number of points in the collection.
    ///
    /// # Errors
    /// [`StoreError::Request`] when Qdrant refuses or cannot be reached.
    pub async fn count(&self) -> Result<u64, StoreError> {
        let reply = self
            .collection
            .client
            .count(CountPointsBuilder::new(self.collection.name.as_str()).exact(true))
            .await
            .map_err(|source| self.collection.request_error("count points", source))?;
        Ok(reply.result.map_or(0, |result| result.count))
    }

    /// The exact number of points of one document. A document with no points gives `0`.
    ///
    /// # Errors
    /// [`StoreError::Request`] when Qdrant refuses, cannot be reached, or has no such collection.
    pub async fn count_document(&self, document: DocId) -> Result<u64, StoreError> {
        let reply = self
            .collection
            .client
            .count(
                CountPointsBuilder::new(self.collection.name.as_str())
                    .filter(points_of(document))
                    .exact(true),
            )
            .await
            .map_err(|source| {
                self.collection
                    .request_error("count the points of the document", source)
            })?;
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
        // SMELL: Qdrant does not say how many points a delete removed, so they are counted first.
        // Points that another program stores between the two calls are removed but not counted.
        let count = self.count_document(document).await?;
        self.collection
            .client
            .delete_points(
                DeletePointsBuilder::new(self.collection.name.as_str())
                    .points(points_of(document))
                    .wait(true),
            )
            .await
            .map_err(|source| {
                self.collection
                    .request_error("delete the points of the document", source)
            })?;
        Ok(count)
    }

    /// Makes `labels` the labels of every point of the document: a label that is there is written,
    /// and one that is missing is taken off the point. Nothing else of a point changes, and no
    /// vector is touched. A document with no points is not an error.
    ///
    /// # Errors
    /// [`StoreError::Request`] when Qdrant refuses or cannot be reached.
    pub async fn set_document_labels(
        &self,
        document: DocId,
        labels: &DocumentLabels,
    ) -> Result<(), StoreError> {
        let fields = [
            (MEDIA_FIELD, labels.media.as_deref().map(Value::from)),
            (
                CATEGORY_FIELD,
                labels
                    .category
                    .map(|category| Value::from(category.as_str())),
            ),
            (
                AUTHORS_FIELD,
                (!labels.authors.is_empty()).then(|| Value::from(labels.authors.clone())),
            ),
            (MEDIA_TAGS_FIELD, tag_list(&labels.media_tags)),
            (TAGS_FIELD, tag_list(&labels.tags)),
        ];
        let mut given = Map::new();
        let mut left_out = Vec::new();
        for (field, value) in fields {
            match value {
                Some(value) => {
                    given.insert(field.to_owned(), value);
                }
                None => left_out.push(field.to_owned()),
            }
        }
        if !left_out.is_empty() {
            self.collection
                .client
                .delete_payload(
                    DeletePayloadPointsBuilder::new(self.collection.name.as_str(), left_out)
                        .points_selector(points_of(document))
                        .wait(true),
                )
                .await
                .map_err(|source| {
                    self.collection
                        .request_error("remove labels from the document", source)
                })?;
        }
        if !given.is_empty() {
            self.collection
                .client
                .set_payload(
                    SetPayloadPointsBuilder::new(
                        self.collection.name.as_str(),
                        Payload::from(given),
                    )
                    .points_selector(points_of(document))
                    .wait(true),
                )
                .await
                .map_err(|source| {
                    self.collection
                        .request_error("write labels to the document", source)
                })?;
        }
        Ok(())
    }
}

fn tag_list(tags: &BTreeSet<Tag>) -> Option<Value> {
    let tags: Vec<&str> = tags.iter().map(Tag::as_str).collect();
    (!tags.is_empty()).then(|| Value::from(tags))
}

fn points_of(document: DocId) -> Filter {
    Filter::must([Condition::matches(DOC_ID_FIELD, document.to_string())])
}

fn point_struct(point: &ItemPoint) -> Result<PointStruct, StoreError> {
    let json = serde_json::to_value(&point.payload).map_err(|source| StoreError::ItemPayload {
        id: point.id,
        source,
    })?;
    let payload =
        Payload::try_from(json).map_err(|_| StoreError::ItemPayloadShape { id: point.id })?;
    Ok(PointStruct::new(
        point.id.to_string(),
        point.vector.clone(),
        payload,
    ))
}
