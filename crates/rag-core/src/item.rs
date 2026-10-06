//! What an item is, how it is told apart from every other item, and what is stored with it.
//! Identifiers are computed from the source, never drawn at random, so ingesting the same
//! chapter twice names the same points.

use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Fixed for good: changing it gives every document and item a new identifier.
const DOCUMENT_NAMESPACE: Uuid = Uuid::from_u128(0x73ca4e6a_716f_46fb_bc79_7c63fc5612d7);

/// One chapter folder. Made from the SHA-256 of the source PDF, so the same PDF is always the
/// same document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DocId(Uuid);

impl DocId {
    /// The hash is used as given: it is already a SHA-256, so it is not hashed again.
    pub fn from_source_sha256(source_sha256: &str) -> DocId {
        DocId(Uuid::new_v5(&DOCUMENT_NAMESPACE, source_sha256.as_bytes()))
    }
}

impl fmt::Display for DocId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for DocId {
    type Err = ParseDocIdError;

    /// Reads what `Display` prints: the UUID of a document.
    fn from_str(text: &str) -> Result<DocId, ParseDocIdError> {
        Uuid::parse_str(text)
            .map(DocId)
            .map_err(|source| ParseDocIdError {
                text: text.to_owned(),
                source,
            })
    }
}

/// The text is not the UUID of a document. The message is written to be read on its own,
/// because a command line prints only this line.
#[derive(thiserror::Error, Debug)]
#[error(
    "{text:?} is not a document id; a document id is the UUID that an ingest prints as \"document id\""
)]
pub struct ParseDocIdError {
    text: String,
    #[source]
    source: uuid::Error,
}

/// One stored thing. An item has this one identifier in every store that holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ItemId(Uuid);

impl ItemId {
    /// `position` is the item's index in the document's items in reading order, from 0.
    pub fn new(document: DocId, kind: ItemKind, position: u32) -> ItemId {
        let name = format!("{}:{position}", kind.as_str());
        ItemId(Uuid::new_v5(&document.0, name.as_bytes()))
    }
}

impl fmt::Display for ItemId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for ItemId {
    type Err = uuid::Error;

    fn from_str(text: &str) -> Result<ItemId, uuid::Error> {
        text.parse().map(ItemId)
    }
}

/// The four things a chapter is stored as. A heading is not one of them: it is context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemKind {
    Chunk,
    Formula,
    Figure,
    Table,
}

impl ItemKind {
    /// Every kind, in the order of the enum.
    // SMELL: a kind that is added to the enum must be added to this list by hand. Nothing checks
    // it, and a kind that is missing here is refused as unknown.
    pub const ALL: [ItemKind; 4] = [
        ItemKind::Chunk,
        ItemKind::Formula,
        ItemKind::Figure,
        ItemKind::Table,
    ];

    /// The name stored in the payload and used to make the item's identifier.
    pub fn as_str(self) -> &'static str {
        match self {
            ItemKind::Chunk => "chunk",
            ItemKind::Formula => "formula",
            ItemKind::Figure => "figure",
            ItemKind::Table => "table",
        }
    }
}

/// The text that was given as a kind is not the name of any kind.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
#[error(
    "`{given}` is not a kind of item; the kinds are {kinds}",
    kinds = ItemKind::ALL.map(ItemKind::as_str).join(", ")
)]
pub struct UnknownItemKind {
    given: String,
}

impl FromStr for ItemKind {
    type Err = UnknownItemKind;

    fn from_str(text: &str) -> Result<ItemKind, UnknownItemKind> {
        ItemKind::ALL
            .into_iter()
            .find(|kind| kind.as_str() == text)
            .ok_or_else(|| UnknownItemKind {
                given: text.to_owned(),
            })
    }
}

/// What is stored beside an item's vector, and what search reads back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemPayload {
    pub doc_id: DocId,
    pub doc_title: String,
    /// The page's position in the chapter, from 1. Use it to put items in order.
    pub page: u32,
    /// The page number as printed, such as "147" or "xii". Use it to cite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub printed_page: Option<String>,
    pub kind: ItemKind,
    /// The item as the reader would quote it: the raw LaTeX for a formula, the Markdown body for
    /// a table, the explanation for a figure.
    pub text: String,
    /// The figure's own picture. Figures only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_path: Option<PathBuf>,
}
