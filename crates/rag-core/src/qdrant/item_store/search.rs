//! Finds items by their vectors: among all items, among chosen ones, and among the items of a
//! document that carry chosen printed labels.

use qdrant_client::Payload;
use qdrant_client::qdrant::vector_output::Vector;
use qdrant_client::qdrant::{
    Condition, Filter, GetPointsBuilder, PointId, QueryPointsBuilder, ScoredPoint,
};

use super::{DOC_ID_FIELD, ItemStore, KIND_FIELD};
use crate::qdrant::point_id_text;
use crate::{DocId, Embedding, ItemId, ItemKind, ItemPayload, StoreError};

#[derive(Debug, Clone, PartialEq)]
pub struct ItemHit {
    pub id: ItemId,
    /// The cosine similarity to the question: higher is nearer.
    pub score: f32,
    pub payload: ItemPayload,
}

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
    /// `None` is every item, and an empty list is no item, so it finds nothing.
    pub items: Option<Vec<ItemId>>,
}

impl ItemFilter {
    fn conditions(&self) -> Vec<Condition> {
        let of_documents = self.documents.as_ref().map(|documents| {
            let ids: Vec<String> = documents.iter().map(ToString::to_string).collect();
            Condition::matches(DOC_ID_FIELD, ids)
        });
        let of_items = self
            .items
            .as_ref()
            .map(|items| Condition::has_id(items.iter().map(ToString::to_string)));
        self.kind
            .map(kind_condition)
            .into_iter()
            .chain(of_documents)
            .chain(of_items)
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
            Vec::new(),
            limit as u64,
        )
        .await
    }

    /// The `limit` items whose vectors are nearest to the stored vector of the item `id`, nearest
    /// first, among the items that the filter lets through. The item itself is never among them.
    /// `None` when the collection holds no point with this id. It embeds nothing, and it never
    /// creates the collection.
    ///
    /// # Errors
    /// - [`StoreError::Request`] when Qdrant refuses, cannot be reached, or has no such collection
    /// - [`StoreError::ItemVector`] when the point has no vector of numbers
    /// - [`StoreError::ItemPointId`] and [`StoreError::StoredItemPayload`] when a stored point is not an
    ///   item; one such point fails the whole search
    pub async fn search_by_point(
        &self,
        id: ItemId,
        filter: &ItemFilter,
        limit: usize,
    ) -> Result<Option<Vec<ItemHit>>, StoreError> {
        let Some(vector) = self.stored_vector(id).await? else {
            return Ok(None);
        };
        let not_itself = vec![Condition::has_id([id.to_string()])];
        let hits = self
            .query(
                "search for items near a stored item",
                vector,
                filter.conditions(),
                not_itself,
                limit as u64,
            )
            .await?;
        Ok(Some(hits))
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
        self.query(
            "rank items",
            vector,
            conditions,
            Vec::new(),
            ids.len() as u64,
        )
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
        // SMELL: a citation in another form than the label, such as "Fig. 7-2" for "Figure 7-2",
        // finds nothing, and two items of one document with the same label are both pulled in.
        // Matching other forms would need every point to hold its label in one fixed form.
        let conditions = vec![
            Condition::matches(DOC_ID_FIELD, document.to_string()),
            Condition::matches(LABEL_FIELD, labels.to_vec()),
        ];
        self.query(
            "look up the items that carry these labels",
            vector,
            conditions,
            Vec::new(),
            LABELLED_LIMIT,
        )
        .await
    }

    /// The vector of the point, or `None` when no point has the id.
    async fn stored_vector(&self, id: ItemId) -> Result<Option<Embedding>, StoreError> {
        let request = GetPointsBuilder::new(
            self.collection.name.as_str(),
            vec![PointId::from(id.to_string())],
        )
        .with_payload(false)
        .with_vectors(true);
        let reply = self
            .collection
            .client
            .get_points(request)
            .await
            .map_err(self.collection.request_error("read the vector of an item"))?;
        let Some(point) = reply.result.into_iter().next() else {
            return Ok(None);
        };
        match point.vectors.and_then(|vectors| vectors.get_vector()) {
            Some(Vector::Dense(dense)) => Ok(Some(dense.data)),
            _ => Err(StoreError::ItemVector {
                id,
                collection: self.collection.name.clone(),
            }),
        }
    }

    async fn query(
        &self,
        action: &'static str,
        vector: Embedding,
        conditions: Vec<Condition>,
        must_not: Vec<Condition>,
        limit: u64,
    ) -> Result<Vec<ItemHit>, StoreError> {
        let mut query = QueryPointsBuilder::new(self.collection.name.as_str())
            .query(vector)
            .limit(limit)
            .with_payload(true);
        if !conditions.is_empty() || !must_not.is_empty() {
            query = query.filter(Filter {
                must: conditions,
                must_not,
                ..Filter::default()
            });
        }
        let reply = self
            .collection
            .client
            .query(query)
            .await
            .map_err(self.collection.request_error(action))?;
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
