//! The read of every document with the mark that it is ingested whole, its folder and how many
//! items of each kind it has. It is one statement, so all of it is read at the same moment.

use std::path::PathBuf;

use falkordb::FalkorValue;
use rag_core::ItemKind;

use super::FalkorGraph;
use super::reads::{DOCUMENT_COLUMNS, document_from_row, unreadable_reply};
use crate::contents::{DocumentRecord, ItemsByKind};
use crate::store::GraphError;

const RECORD_ROW: &str = "\
a document with its mark, its folder and its items by kind (an id, a title, a media or null, a \
category or null, a list of authors or null, a list of media tags or null, a list of tags or null, \
a count of items or null, a folder or null and a list of pairs of a kind of item and a count that \
is not negative)";

// A document with no items has one pair, with no kind and a count of zero.
fn document_records_statement() -> String {
    format!(
        "\
MATCH (d:Document)
OPTIONAL MATCH (d)-[:HAS_ITEM]->(i:Item)
WITH d, i.kind AS kind, count(i) AS items
WITH d, collect([kind, items]) AS items_by_kind
RETURN {DOCUMENT_COLUMNS}, d.ingested_items, d.chapter_folder, items_by_kind
ORDER BY d.title, d.id"
    )
}

pub(super) async fn document_records(
    graph: &FalkorGraph,
) -> Result<Vec<DocumentRecord>, GraphError> {
    let action = "read the documents with their marks, their folders and their items by kind";
    let reply = graph
        .run(action, &document_records_statement(), Vec::new())
        .await?;
    reply
        .data
        .into_values_lossy()
        .map(record_from_row)
        .collect::<Result<_, _>>()
        .map_err(|found| unreadable_reply(graph, action, RECORD_ROW, found))
}

fn count_of(items: &mut ItemsByKind, kind: ItemKind) -> &mut u64 {
    match kind {
        ItemKind::Chunk => &mut items.chunks,
        ItemKind::Formula => &mut items.formulas,
        ItemKind::Figure => &mut items.figures,
        ItemKind::Table => &mut items.tables,
    }
}

fn record_from_row(row: Vec<FalkorValue>) -> Result<DocumentRecord, String> {
    let row = <[FalkorValue; 10]>::try_from(row).map_err(|row| format!("{row:?}"))?;
    let [
        id,
        title,
        media,
        category,
        authors,
        media_tags,
        tags,
        mark,
        folder,
        items_by_kind,
    ] = row;
    let node = document_from_row(vec![id, title, media, category, authors, media_tags, tags])?;
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
    let FalkorValue::Array(pairs) = items_by_kind else {
        return Err(format!("{items_by_kind:?}"));
    };
    let mut items = ItemsByKind::default();
    for pair in pairs {
        if let Some((kind, count)) = items_of_kind_from_pair(pair)? {
            *count_of(&mut items, kind) = count;
        }
    }
    Ok(DocumentRecord {
        node,
        ingested_items,
        chapter_folder,
        items,
    })
}

/// The pair of a document with no items, which has no kind and a count of zero, gives `None`.
fn items_of_kind_from_pair(pair: FalkorValue) -> Result<Option<(ItemKind, u64)>, String> {
    let FalkorValue::Array(pair) = pair else {
        return Err(format!("{pair:?}"));
    };
    match <[FalkorValue; 2]>::try_from(pair).map_err(|pair| format!("{pair:?}"))? {
        [FalkorValue::None, FalkorValue::I64(0)] => Ok(None),
        [FalkorValue::String(kind), FalkorValue::I64(count)] => Ok(Some((
            kind.parse().map_err(|_| format!("{kind:?}"))?,
            u64::try_from(count).map_err(|_| count.to_string())?,
        ))),
        pair => Err(format!("{pair:?}")),
    }
}
