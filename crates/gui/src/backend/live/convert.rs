//! The ids, the item kind, the category, the document name, the label shapes and what a run used,
//! to and from the types of the backend. Every part of the live backend converts through these and
//! writes none of its own.

use std::collections::BTreeSet;

use graph::MediaNode;
use rag_core::{LabelFilter, MediaLabels, Tag, UsageTally, author_list};
use rag_ingestion::{MediaChange, TagChange};
use uuid::Uuid;

use crate::contract::{
    Category, ConceptId, DocId, DocumentName, DocumentTagsEdit, Filters, ItemId, ItemKind,
    MediaEdit, ModelTokens, NewMedia, Usage,
};

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

impl From<rag_core::Category> for Category {
    fn from(category: rag_core::Category) -> Category {
        match category {
            rag_core::Category::Book => Category::Book,
            rag_core::Category::Paper => Category::Paper,
            rag_core::Category::Other => Category::Other,
        }
    }
}

impl From<Category> for rag_core::Category {
    fn from(category: Category) -> rag_core::Category {
        match category {
            Category::Book => rag_core::Category::Book,
            Category::Paper => rag_core::Category::Paper,
            Category::Other => rag_core::Category::Other,
        }
    }
}

impl From<ocr::DocumentName> for DocumentName {
    fn from(name: ocr::DocumentName) -> DocumentName {
        match name {
            ocr::DocumentName::Chapter { number, name } => DocumentName::Chapter { number, name },
            ocr::DocumentName::Title(title) => DocumentName::Title(title),
        }
    }
}

impl From<&DocumentName> for ocr::DocumentName {
    fn from(name: &DocumentName) -> ocr::DocumentName {
        match name {
            DocumentName::Chapter { number, name } => ocr::DocumentName::Chapter {
                number: *number,
                name: name.clone(),
            },
            DocumentName::Title(title) => ocr::DocumentName::Title(title.clone()),
        }
    }
}

/// The tags that are not blank, in the form the backend stores them.
fn tags(texts: &[String]) -> impl Iterator<Item = Tag> {
    texts.iter().filter_map(|text| text.parse().ok())
}

impl From<&Filters> for LabelFilter {
    fn from(filters: &Filters) -> LabelFilter {
        LabelFilter {
            media: filters.media.clone(),
            author: filters.author.clone(),
            category: filters.category.map(Into::into),
            tags: tags(&filters.tags).collect::<BTreeSet<_>>(),
        }
    }
}

/// A blank tag is dropped.
impl From<&DocumentTagsEdit> for TagChange {
    fn from(edit: &DocumentTagsEdit) -> TagChange {
        TagChange {
            add: tags(&edit.add).collect(),
            remove: tags(&edit.remove).collect(),
        }
    }
}

/// The title is trimmed, and a blank author or tag is dropped.
impl From<&NewMedia> for MediaNode {
    fn from(media: &NewMedia) -> MediaNode {
        MediaNode {
            title: media.title.trim().to_owned(),
            labels: MediaLabels {
                category: media.category.into(),
                authors: author_list(&media.authors),
                tags: tags(&media.tags).collect(),
            },
        }
    }
}

/// An edit gives the whole new state, so every label is replaced.
impl From<&MediaEdit> for MediaChange {
    fn from(edit: &MediaEdit) -> MediaChange {
        MediaChange {
            category: Some(edit.category.into()),
            authors: Some(edit.authors.clone()),
            tags: Some(tags(&edit.tags).collect()),
        }
    }
}

impl From<&UsageTally> for Usage {
    fn from(tally: &UsageTally) -> Usage {
        let models = tally
            .models()
            .map(|(model, used)| ModelTokens {
                model: model.to_owned(),
                input: used.tokens.input_tokens,
                output: used.tokens.output_tokens,
                cache_read: used.tokens.cache_read_tokens,
                cache_write: used.tokens.cache_write_tokens,
                estimated: used.estimated,
            })
            .collect();
        Usage {
            models,
            cost_usd: tally.cost_usd(),
        }
    }
}
