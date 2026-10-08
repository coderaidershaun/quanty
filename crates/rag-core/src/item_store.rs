//! The Qdrant collection that holds every item: one unnamed vector and the payload of an item
//! for each point. Ingestion writes it and retrieval reads it, so both use this one definition.

use qdrant_client::Payload;
use qdrant_client::qdrant::{
    Condition, CountPointsBuilder, CreateFieldIndexCollectionBuilder, DeletePayloadPointsBuilder,
    DeletePointsBuilder, FieldType, Filter, PointStruct, QueryPointsBuilder, ScoredPoint,
    SetPayloadPointsBuilder, UpsertPointsBuilder,
};
use serde_json::{Map, Value};

use crate::qdrant::{Collection, point_id_text};
use crate::{
    Config, DocId, DocumentLabels, Embedding, ItemId, ItemKind, ItemPayload, StoreError, Tag,
};

/// A group of this many points with their vectors is about a megabyte, well inside a request.
const UPSERT_GROUP_SIZE: usize = 256;

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
        // SMELL: counting and deleting are two calls, so points of the document that another
        // program stores between the two are removed but not counted, and the number returned
        // is then too low.
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
        let tags: Vec<&str> = labels.tags.iter().map(Tag::as_str).collect();
        let fields = [
            (BOOK_FIELD, labels.book.as_deref().map(Value::from)),
            (AUTHOR_FIELD, labels.author.as_deref().map(Value::from)),
            (TAGS_FIELD, (!tags.is_empty()).then(|| Value::from(tags))),
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

/// The name of the payload field that holds the document of an item. It must match the field name
/// of `ItemPayload`.
const DOC_ID_FIELD: &str = "doc_id";

/// The names of the payload fields that hold the labels of a document. They must match the field
/// names of `DocumentLabels`.
const BOOK_FIELD: &str = "book";
const AUTHOR_FIELD: &str = "author";
const TAGS_FIELD: &str = "tags";

/// The name of the payload field that holds the printed label of an item. It must match the field
/// name of `ItemPayload`.
const LABEL_FIELD: &str = "label";

/// The most items that one lookup by label gives back; the rest are left out. A chunk cites a few
/// figures, tables and equations, so this leaves room for every one of them, and for a label that
/// two items share.
const LABELLED_LIMIT: u64 = 32;

/// The default is every item.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemFilter {
    pub kind: Option<ItemKind>,
    /// `None` is every document, and an empty list is no document, so it finds nothing.
    pub documents: Option<Vec<DocId>>,
}

impl ItemFilter {
    fn conditions(&self) -> Vec<Condition> {
        let of_documents = self.documents.as_ref().map(|documents| {
            let ids: Vec<String> = documents.iter().map(ToString::to_string).collect();
            Condition::matches(DOC_ID_FIELD, ids)
        });
        self.kind
            .map(kind_condition)
            .into_iter()
            .chain(of_documents)
            .collect()
    }
}

impl ItemStore {
    /// The `limit` items whose vectors are nearest to `vector`, nearest first, among the items that
    /// the filter lets through. It never creates the collection.
    ///
    /// # Errors
    /// - [`StoreError::Request`] when Qdrant refuses, cannot be reached, or has no such collection
    /// - [`StoreError::ItemPointId`] and [`StoreError::StoredItemPayload`] when a stored point is not an
    ///   item; one such point fails the whole search
    pub async fn search(
        &self,
        vector: Embedding,
        filter: &ItemFilter,
        limit: usize,
    ) -> Result<Vec<ItemHit>, StoreError> {
        self.query(
            "search for items",
            vector,
            filter.conditions(),
            limit as u64,
        )
        .await
    }

    /// Every stored item among `ids` that the filter lets through, nearest to `vector` first. An
    /// id that is not stored is left out. Empty `ids` make no call. It never creates the
    /// collection.
    ///
    /// # Errors
    /// - [`StoreError::Request`] when Qdrant refuses, cannot be reached, or has no such collection
    /// - [`StoreError::ItemPointId`] and [`StoreError::StoredItemPayload`] when a stored point is not an
    ///   item; one such point fails the whole search
    pub async fn rank(
        &self,
        vector: Embedding,
        ids: &[ItemId],
        filter: &ItemFilter,
    ) -> Result<Vec<ItemHit>, StoreError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let of_ids = Condition::has_id(ids.iter().map(ToString::to_string));
        let conditions = std::iter::once(of_ids).chain(filter.conditions()).collect();
        self.query("rank items", vector, conditions, ids.len() as u64)
            .await
    }

    /// The items of `document` whose printed label is one of `labels`, nearest to `vector` first.
    /// Empty `labels` make no call. It never creates the collection.
    ///
    /// # Errors
    /// - [`StoreError::Request`] when Qdrant refuses, cannot be reached, or has no such collection
    /// - [`StoreError::ItemPointId`] and [`StoreError::StoredItemPayload`] when a stored point is not an
    ///   item; one such point fails the whole search
    pub async fn labelled(
        &self,
        vector: Embedding,
        document: DocId,
        labels: &[String],
    ) -> Result<Vec<ItemHit>, StoreError> {
        if labels.is_empty() {
            return Ok(Vec::new());
        }
        // The labels go as a list even when there is one, because a single text that holds a
        // space, such as `Figure 13-4`, is matched as full text, and a list is matched exactly.
        //
        // SMELL: the label must be printed the same way in both places. A citation that is
        // printed in another form, such as "Fig. 7-2" for "Figure 7-2", finds nothing. Two items
        // of one document that have the same label are both pulled in.
        let conditions = vec![
            Condition::matches(DOC_ID_FIELD, document.to_string()),
            Condition::matches(LABEL_FIELD, labels.to_vec()),
        ];
        self.query(
            "look up the items that carry these labels",
            vector,
            conditions,
            LABELLED_LIMIT,
        )
        .await
    }

    async fn query(
        &self,
        action: &'static str,
        vector: Embedding,
        conditions: Vec<Condition>,
        limit: u64,
    ) -> Result<Vec<ItemHit>, StoreError> {
        let mut query = QueryPointsBuilder::new(self.collection.name.as_str())
            .query(vector)
            .limit(limit)
            .with_payload(true);
        if !conditions.is_empty() {
            query = query.filter(Filter::must(conditions));
        }
        let reply = self
            .collection
            .client
            .query(query)
            .await
            .map_err(|source| self.collection.request_error(action, source))?;
        reply
            .result
            .into_iter()
            .map(|point| item_hit(&self.collection.name, point))
            .collect()
    }
}

fn kind_condition(kind: ItemKind) -> Condition {
    Condition::matches(KIND_FIELD, kind.as_str().to_owned())
}

fn item_hit(collection: &str, point: ScoredPoint) -> Result<ItemHit, StoreError> {
    let id_text = point_id_text(point.id);
    let id = id_text
        .parse::<ItemId>()
        .map_err(|source| StoreError::ItemPointId {
            collection: collection.to_owned(),
            point: id_text,
            source,
        })?;
    let json = serde_json::Value::from(Payload::from(point.payload));
    let payload = serde_json::from_value::<ItemPayload>(json).map_err(|source| {
        StoreError::StoredItemPayload {
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
