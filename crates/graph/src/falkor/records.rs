//! The read of every document with the mark that it is ingested whole, its folder and how many
//! items of each kind it has. It is one statement, so all of it is read at the same moment.

use std::path::PathBuf;

use falkordb::FalkorValue;
use rag_core::ItemKind;

use super::documents::{DOCUMENT_COLUMNS, count_or_null, document_from_row, text_or_null};
use super::{FalkorGraph, read_rows};
use crate::contents::{DocumentRecord, ItemsByKind};
use crate::error::GraphError;

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

impl FalkorGraph {
    /// Every document, each once, ordered by title and then by id, with its labels, the mark that
    /// it is ingested whole, the folder it was ingested from and how many items of each kind it
    /// has. A document with no items has zero of each kind. This read counts every item in the
    /// graph, so [`FalkorGraph::documents`] is the read to use when the title and the labels are
    /// enough, and [`FalkorGraph::chapter_folders`] when the folders are.
    ///
    /// # Errors
    /// - [`GraphError::Query`] when the store refuses or cannot be reached
    /// - [`GraphError::UnreadableReply`] when the store answers with something that is not a
    ///   document with its mark and its folder, or not a count of items of one kind
    pub async fn document_records(&self) -> Result<Vec<DocumentRecord>, GraphError> {
        let action = "read the documents with their marks, their folders and their items by kind";
        let reply = self
            .run(action, &document_records_statement(), Vec::new())
            .await?;
        read_rows(self, action, RECORD_ROW, reply, record_from_row)
    }
}

fn count_of(items: &mut ItemsByKind, kind: ItemKind) -> &mut u64 {
    match kind {
        ItemKind::Chunk => &mut items.chunks,
        ItemKind::Formula => &mut items.formulas,
        ItemKind::Figure => &mut items.figures,
        ItemKind::Table => &mut items.tables,
        ItemKind::Code => &mut items.code,
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
    let ingested_items = count_or_null(mark)?;
    let chapter_folder = text_or_null(folder)?.map(PathBuf::from);
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
