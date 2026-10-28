//! The statements that read where concepts are mentioned: the documents and the items that
//! mention a concept, and the concepts that documents share.

use std::collections::HashMap;

use falkordb::FalkorValue;
use rag_core::{ConceptId, DocId, ItemId, ItemKind};

use super::documents::text_or_null;
use super::{FalkorGraph, fixed, id_list, id_value, parsed, read_rows};
use crate::contents::{ConceptDocument, ConceptItem, ConceptNode, ItemPlace, SharedConcept};
use crate::error::GraphError;

const CONCEPT_DOCUMENT_ROW: &str = "\
a document with a count (an id, a title and one whole number that is not negative)";
const SHARED_CONCEPT_ROW: &str = "\
a concept with its counts (an id, a name, a normalised name, a definition and a list of pairs of \
a document id and a whole number that is not negative)";
const CONCEPT_ITEM_ROW: &str = "\
an item that mentions a concept (an item id, a document id, a page, a printed page or null, a \
kind of item and a wording)";
const ITEM_PLACE_ROW: &str = "an item with its place (an item id, a document id and a page)";

const DOCUMENTS_FOR_CONCEPT: &str = "\
MATCH (d:Document)-[:HAS_ITEM]->(i:Item)-[:MENTIONS]->(:Concept {id: $concept})
WITH d, count(i) AS mentions
RETURN d.id, d.title, mentions
ORDER BY mentions DESC, d.title, d.id";

const CONCEPTS_SHARED: &str = "\
MATCH (d:Document)-[:HAS_ITEM]->(i:Item)-[:MENTIONS]->(c:Concept)
WHERE d.id IN $documents
WITH c, d, count(i) AS mentions
WITH c, collect([d.id, mentions]) AS per_document, sum(mentions) AS total
RETURN c.id, c.name, c.normalised_name, c.definition, per_document
ORDER BY total DESC, c.normalised_name, c.id";

// A null parameter turns its filter off. The order puts the document by title first, and its id
// breaks the tie of two documents with one title, so that the pages of one document stay together.
const ITEMS_MENTIONING: &str = "\
MATCH (d:Document)-[:HAS_ITEM]->(i:Item)-[m:MENTIONS]->(:Concept {id: $concept})
WHERE ($document IS NULL OR d.id = $document) AND ($kind IS NULL OR i.kind = $kind)
RETURN i.id, d.id, i.page, i.printed_page, i.kind, m.wording
ORDER BY d.title, d.id, i.page, i.kind, i.id
SKIP $offset LIMIT $limit";

const ITEM_PLACES: &str = "\
MATCH (d:Document)-[:HAS_ITEM]->(i:Item)
WHERE i.id IN $items
RETURN i.id, d.id, i.page";

impl FalkorGraph {
    /// The documents that have an item mentioning the concept, each with how many of its items do.
    /// The documents with the most mentions come first, then by title. A concept that is not in
    /// the graph, or that no item mentions, gives none.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a
    ///   document with a count
    pub async fn documents_for_concept(
        &self,
        concept: ConceptId,
    ) -> Result<Vec<ConceptDocument>, GraphError> {
        let action = "read the documents whose items mention the concept";
        let parameters = vec![("concept", id_value(concept))];
        let reply = self.run(action, DOCUMENTS_FOR_CONCEPT, parameters).await?;
        read_rows(
            self,
            action,
            CONCEPT_DOCUMENT_ROW,
            reply,
            concept_document_from_row,
        )
    }

    /// Every concept that the items of at least one of these documents mention, each once, with
    /// how many items of each of those documents mention it, ordered by document id. A document
    /// with no item that mentions the concept is not in its list. The concepts with the most
    /// mentions in all come first, then by normalised name. An empty slice makes no call.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a
    ///   concept with its counts
    pub async fn concepts_shared(
        &self,
        documents: &[DocId],
    ) -> Result<Vec<SharedConcept>, GraphError> {
        if documents.is_empty() {
            return Ok(Vec::new());
        }
        let action = "read the concepts that the items of the documents mention";
        let parameters = vec![("documents", id_list(documents))];
        let reply = self.run(action, CONCEPTS_SHARED, parameters).await?;
        read_rows(
            self,
            action,
            SHARED_CONCEPT_ROW,
            reply,
            shared_concept_from_row,
        )
    }

    /// The items that mention the concept, with where each one is and the name it used. They are
    /// ordered by the title of their document, then by page, then by kind, then by item id. Give
    /// `document` or `kind` to keep only the items of that document or of that kind. `offset`
    /// items are skipped, and at most `limit` come back.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not an item
    ///   with its place
    pub async fn items_mentioning(
        &self,
        concept: ConceptId,
        document: Option<DocId>,
        kind: Option<ItemKind>,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<ConceptItem>, GraphError> {
        let action = "read the items that mention the concept";
        let parameters = vec![
            ("concept", id_value(concept)),
            ("document", document.map_or(FalkorValue::None, id_value)),
            (
                "kind",
                kind.map_or(FalkorValue::None, |kind| {
                    FalkorValue::String(kind.as_str().to_owned())
                }),
            ),
            ("offset", count_value(offset)),
            ("limit", count_value(limit)),
        ];
        let reply = self.run(action, ITEMS_MENTIONING, parameters).await?;
        read_rows(self, action, CONCEPT_ITEM_ROW, reply, concept_item_from_row)
    }

    /// The document and the page of each of these items. An item that the graph does not hold is
    /// left out. An empty slice makes no call.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not an item
    ///   with its place
    pub async fn item_places(
        &self,
        items: &[ItemId],
    ) -> Result<HashMap<ItemId, ItemPlace>, GraphError> {
        if items.is_empty() {
            return Ok(HashMap::new());
        }
        let action = "read where the items are";
        let parameters = vec![("items", id_list(items))];
        let reply = self.run(action, ITEM_PLACES, parameters).await?;
        read_rows(self, action, ITEM_PLACE_ROW, reply, item_place_from_row)
    }
}

pub(super) fn count_value(count: usize) -> FalkorValue {
    FalkorValue::I64(i64::try_from(count).unwrap_or(i64::MAX))
}

fn count_of(value: FalkorValue) -> Result<u64, String> {
    let FalkorValue::I64(count) = value else {
        return Err(format!("{value:?}"));
    };
    u64::try_from(count).map_err(|_| count.to_string())
}

fn concept_document_from_row(row: Vec<FalkorValue>) -> Result<ConceptDocument, String> {
    let row = fixed::<3>(row)?;
    let [
        FalkorValue::String(document),
        FalkorValue::String(title),
        mentions,
    ] = row
    else {
        return Err(format!("{row:?}"));
    };
    Ok(ConceptDocument {
        document: parsed(&document)?,
        title,
        mentions: count_of(mentions)?,
    })
}

fn shared_concept_from_row(row: Vec<FalkorValue>) -> Result<SharedConcept, String> {
    let row = fixed::<5>(row)?;
    let [
        FalkorValue::String(id),
        FalkorValue::String(name),
        FalkorValue::String(normalised_name),
        FalkorValue::String(definition),
        FalkorValue::Array(pairs),
    ] = row
    else {
        return Err(format!("{row:?}"));
    };
    let mut mentions = pairs
        .into_iter()
        .map(document_count_from_pair)
        .collect::<Result<Vec<_>, _>>()?;
    mentions.sort();
    Ok(SharedConcept {
        concept: ConceptNode {
            id: parsed(&id)?,
            name,
            normalised_name,
            definition,
        },
        mentions,
    })
}

fn document_count_from_pair(pair: FalkorValue) -> Result<(DocId, u64), String> {
    let FalkorValue::Array(pair) = pair else {
        return Err(format!("{pair:?}"));
    };
    match <[FalkorValue; 2]>::try_from(pair).map_err(|pair| format!("{pair:?}"))? {
        [FalkorValue::String(document), count] => Ok((parsed(&document)?, count_of(count)?)),
        pair => Err(format!("{pair:?}")),
    }
}

fn concept_item_from_row(row: Vec<FalkorValue>) -> Result<ConceptItem, String> {
    let row = fixed::<6>(row)?;
    let [
        FalkorValue::String(item),
        FalkorValue::String(document),
        FalkorValue::I64(page),
        printed_page,
        FalkorValue::String(kind),
        FalkorValue::String(wording),
    ] = row
    else {
        return Err(format!("{row:?}"));
    };
    Ok(ConceptItem {
        item: parsed(&item)?,
        document: parsed(&document)?,
        page: u32::try_from(page).map_err(|_| page.to_string())?,
        printed_page: text_or_null(printed_page)?,
        kind: parsed(&kind)?,
        wording,
    })
}

fn item_place_from_row(row: Vec<FalkorValue>) -> Result<(ItemId, ItemPlace), String> {
    let row = fixed::<3>(row)?;
    let [
        FalkorValue::String(item),
        FalkorValue::String(document),
        FalkorValue::I64(page),
    ] = row
    else {
        return Err(format!("{row:?}"));
    };
    let place = ItemPlace {
        document: parsed(&document)?,
        page: u32::try_from(page).map_err(|_| page.to_string())?,
    };
    Ok((parsed(&item)?, place))
}
