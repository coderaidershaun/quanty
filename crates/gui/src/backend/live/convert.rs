//! The ids, the item kind and the three label shapes, to and from the types of the backend. Every
//! part of the live backend converts through these and writes none of its own.

use std::collections::BTreeSet;

use rag_core::{DocumentLabels, Tag};
use rag_ingestion::LabelChange;
use uuid::Uuid;

use crate::contract::{ConceptId, DocId, Filters, ItemId, ItemKind, LabelEdit, NewBook};

fn uuid_of(text: &str) -> Uuid {
    Uuid::parse_str(text).expect("a backend id prints as a UUID")
}

impl From<rag_core::DocId> for DocId {
    fn from(id: rag_core::DocId) -> DocId {
        DocId(uuid_of(&id.to_string()))
    }
}

impl From<DocId> for rag_core::DocId {
    fn from(id: DocId) -> rag_core::DocId {
        id.0.to_string()
            .parse()
            .expect("the text of a UUID is a document id")
    }
}

impl From<rag_core::ItemId> for ItemId {
    fn from(id: rag_core::ItemId) -> ItemId {
        ItemId(uuid_of(&id.to_string()))
    }
}

impl From<ItemId> for rag_core::ItemId {
    fn from(id: ItemId) -> rag_core::ItemId {
        id.0.to_string()
            .parse()
            .expect("the text of a UUID is an item id")
    }
}

impl From<rag_core::ConceptId> for ConceptId {
    fn from(id: rag_core::ConceptId) -> ConceptId {
        ConceptId(uuid_of(&id.to_string()))
    }
}

impl From<ConceptId> for rag_core::ConceptId {
    fn from(id: ConceptId) -> rag_core::ConceptId {
        id.0.to_string()
            .parse()
            .expect("the text of a UUID is a concept id")
    }
}

impl From<rag_core::ItemKind> for ItemKind {
    fn from(kind: rag_core::ItemKind) -> ItemKind {
        match kind {
            rag_core::ItemKind::Chunk => ItemKind::Chunk,
            rag_core::ItemKind::Formula => ItemKind::Formula,
            rag_core::ItemKind::Figure => ItemKind::Figure,
            rag_core::ItemKind::Table => ItemKind::Table,
        }
    }
}

impl From<ItemKind> for rag_core::ItemKind {
    fn from(kind: ItemKind) -> rag_core::ItemKind {
        match kind {
            ItemKind::Chunk => rag_core::ItemKind::Chunk,
            ItemKind::Formula => rag_core::ItemKind::Formula,
            ItemKind::Figure => rag_core::ItemKind::Figure,
            ItemKind::Table => rag_core::ItemKind::Table,
        }
    }
}

/// The tags that are not blank, in the form the backend stores them.
fn tags(texts: &[String]) -> impl Iterator<Item = Tag> {
    texts.iter().filter_map(|text| text.parse().ok())
}

impl From<&Filters> for DocumentLabels {
    fn from(filters: &Filters) -> DocumentLabels {
        DocumentLabels {
            book: filters.book.clone(),
            author: filters.author.clone(),
            tags: tags(&filters.tags).collect::<BTreeSet<_>>(),
        }
    }
}

impl From<&LabelEdit> for LabelChange {
    fn from(edit: &LabelEdit) -> LabelChange {
        LabelChange {
            author: edit.author.clone(),
            add: tags(&edit.add).collect(),
            remove: tags(&edit.remove).collect(),
        }
    }
}

/// A blank author is none, and a blank tag is dropped.
impl From<&NewBook> for graph::BookNode {
    fn from(book: &NewBook) -> graph::BookNode {
        graph::BookNode {
            title: book.title.trim().to_owned(),
            author: book
                .author
                .as_deref()
                .map(str::trim)
                .filter(|author| !author.is_empty())
                .map(str::to_owned),
            tags: tags(&book.tags).collect(),
        }
    }
}
