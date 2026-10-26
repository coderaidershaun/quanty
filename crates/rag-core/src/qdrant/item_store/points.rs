//! Writes the points of the items: stores them, counts them, deletes the points of a document and
//! writes the labels of a document onto every point of it.

use qdrant_client::Payload;
use qdrant_client::qdrant::{
    Condition, CountPointsBuilder, DeletePayloadPointsBuilder, DeletePointsBuilder, Filter,
    PointStruct, SetPayloadPointsBuilder, UpsertPointsBuilder,
};
use serde_json::Value;

use super::{DOC_ID_FIELD, ItemPoint, ItemStore};
use crate::{DocId, DocumentLabels, StoreError};

/// A group of this many points with their vectors is about a megabyte, well inside a request.
const UPSERT_GROUP_SIZE: usize = 256;

/// The names of the payload fields that hold the labels of a document. They must match the field
/// names of `DocumentLabels`.
const LABEL_FIELDS: [&str; 5] = ["media", "category", "authors", "media_tags", "tags"];

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
                .map_err(self.collection.request_error("store points"))?;
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
            .map_err(self.collection.request_error("count points"))?;
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
            .map_err(
                self.collection
                    .request_error("count the points of the document"),
            )?;
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
            .map_err(
                self.collection
                    .request_error("delete the points of the document"),
            )?;
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
        let Ok(Value::Object(given)) = serde_json::to_value(labels) else {
            unreachable!("labels are a struct of strings and lists, which is a JSON object");
        };
        let left_out: Vec<String> = LABEL_FIELDS
            .into_iter()
            .filter(|field| !given.contains_key(*field))
            .map(str::to_owned)
            .collect();
        if !left_out.is_empty() {
            self.collection
                .client
                .delete_payload(
                    DeletePayloadPointsBuilder::new(self.collection.name.as_str(), left_out)
                        .points_selector(points_of(document))
                        .wait(true),
                )
                .await
                .map_err(
                    self.collection
                        .request_error("remove labels from the document"),
                )?;
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
                .map_err(
                    self.collection
                        .request_error("write labels to the document"),
                )?;
        }
        Ok(())
    }
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
