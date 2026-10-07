//! The read of every document with the mark that it is ingested whole, its folder and how many
//! items of each kind it has. It takes two statements: the documents, and then the counts.

use std::collections::HashMap;
use std::path::PathBuf;

use falkordb::FalkorValue;
use rag_core::{DocId, ItemKind};

use super::FalkorGraph;
use super::reads::{document_from_row, unreadable_reply};
use crate::contents::{DocumentRecord, ItemsByKind};
use crate::store::GraphError;

const RECORD_ROW: &str = "\
a document with its mark and its folder (an id, a title, a book or null, an author or null, \
a list of tags or null, a count of items or null and a folder or null)";
const COUNT_ROW: &str = "\
a count of the items of one kind in one document (a document id, a kind of item and a count \
that is not negative)";

// SMELL: the first five columns must stay the same as in the statement that reads every document
// with only its title and its labels, and in the same order, because one row reader reads both.
const DOCUMENT_RECORDS: &str = "\
MATCH (d:Document)
RETURN d.id, d.title, d.book, d.author, d.tags, d.ingested_items, d.chapter_folder
ORDER BY d.title, d.id";

const ITEMS_BY_KIND: &str = "\
MATCH (d:Document)-[:HAS_ITEM]->(i:Item)
WITH d, i.kind AS kind, count(i) AS items
RETURN d.id, kind, items";

// SMELL: the two statements are not one read. A document that is stored between them is left out,
// and a document that is removed between them is still listed, with no items, until the next read.
pub(super) async fn document_records(
    graph: &FalkorGraph,
) -> Result<Vec<DocumentRecord>, GraphError> {
    let action = "read the documents with their marks and their folders";
    let reply = graph.run(action, DOCUMENT_RECORDS, Vec::new()).await?;
    let mut records = reply
        .data
        .into_values_lossy()
        .map(record_from_row)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|found| unreadable_reply(graph, action, RECORD_ROW, found))?;

    let action = "count the items of each document by kind";
    let reply = graph.run(action, ITEMS_BY_KIND, Vec::new()).await?;
    let mut items_of = HashMap::<DocId, ItemsByKind>::new();
    for row in reply.data.into_values_lossy() {
        let (document, kind, count) = items_by_kind_from_row(row)
            .map_err(|found| unreadable_reply(graph, action, COUNT_ROW, found))?;
        *count_of(items_of.entry(document).or_default(), kind) = count;
    }

    // A count for a document that the first statement did not see is dropped.
    for record in &mut records {
        record.items = items_of.remove(&record.node.id).unwrap_or_default();
    }
    Ok(records)
}

fn count_of(items: &mut ItemsByKind, kind: ItemKind) -> &mut u64 {
    match kind {
        ItemKind::Chunk => &mut items.chunks,
        ItemKind::Formula => &mut items.formulas,
        ItemKind::Figure => &mut items.figures,
        ItemKind::Table => &mut items.tables,
    }
}

/// Reads a row of seven values: the five of a document, then its count of items or null, then its
/// folder or null. The items by kind are left at zero. Any other row comes back as the text that
/// the error shows.
fn record_from_row(row: Vec<FalkorValue>) -> Result<DocumentRecord, String> {
    let row = <[FalkorValue; 7]>::try_from(row).map_err(|row| format!("{row:?}"))?;
    let [id, title, book, author, tags, mark, folder] = row;
    let node = document_from_row(vec![id, title, book, author, tags])?;
    let ingested_items = match mark {
        FalkorValue::None => None,
        FalkorValue::I64(count) => Some(u64::try_from(count).map_err(|_| count.to_string())?),
        other => return Err(format!("{other:?}")),
    };
    let chapter_folder = match folder {
        FalkorValue::None => None,
        FalkorValue::String(text) => Some(PathBuf::from(text)),
        other => return Err(format!("{other:?}")),
    };
    Ok(DocumentRecord {
        node,
        ingested_items,
        chapter_folder,
        items: ItemsByKind::default(),
    })
}

/// Reads a row of a document id, a kind of item and a count that is not negative. Any other row
/// comes back as the text that the error shows.
fn items_by_kind_from_row(row: Vec<FalkorValue>) -> Result<(DocId, ItemKind, u64), String> {
    let row = <[FalkorValue; 3]>::try_from(row).map_err(|row| format!("{row:?}"))?;
    let [
        FalkorValue::String(document),
        FalkorValue::String(kind),
        FalkorValue::I64(count),
    ] = row
    else {
        return Err(format!("{row:?}"));
    };
    Ok((
        document.parse().map_err(|_| format!("{document:?}"))?,
        kind.parse().map_err(|_| format!("{kind:?}"))?,
        u64::try_from(count).map_err(|_| count.to_string())?,
    ))
}
