//! The ids, the item kind, the category, the document name, the label shapes, the steps of a build,
//! what a run used and what became of its insights, to and from the types of the backend. Every
//! part of the live backend converts through these and writes none of its own.

use std::collections::BTreeSet;

pub(crate) trait IntoView<T> {
    fn into_view(self) -> T;
}

use graph::{ConceptNode, MediaNode};
use rag_core::{LabelFilter, MediaLabels, Tag, UsageTally, WantedDocument, author_list};
use rag_ingestion::{MediaChange, TagChange};

use contract::{
    Category, ConceptId, DatasetStage, DocId, DocumentName, DocumentTagsEdit, Filters,
    InsightsOutcome, ItemId, ItemKind, MediaEdit, ModelTokens, NewMedia, PageConcept, Usage,
};

impl IntoView<DocId> for rag_core::DocId {
    fn into_view(self) -> DocId {
        let id = self;
        DocId(id.0)
    }
}

impl IntoView<rag_core::DocId> for DocId {
    fn into_view(self) -> rag_core::DocId {
        let id = self;
        rag_core::DocId(id.0)
    }
}

impl IntoView<ItemId> for rag_core::ItemId {
    fn into_view(self) -> ItemId {
        let id = self;
        ItemId(id.0)
    }
}

impl IntoView<rag_core::ItemId> for ItemId {
    fn into_view(self) -> rag_core::ItemId {
        let id = self;
        rag_core::ItemId(id.0)
    }
}

impl IntoView<ConceptId> for rag_core::ConceptId {
    fn into_view(self) -> ConceptId {
        let id = self;
        ConceptId(id.0)
    }
}

impl IntoView<rag_core::ConceptId> for ConceptId {
    fn into_view(self) -> rag_core::ConceptId {
        let id = self;
        rag_core::ConceptId(id.0)
    }
}

impl IntoView<PageConcept> for ConceptNode {
    fn into_view(self) -> PageConcept {
        let node = self;
        PageConcept {
            id: node.id.into_view(),
            name: node.name,
            definition: node.definition,
        }
    }
}

impl IntoView<ItemKind> for rag_core::ItemKind {
    fn into_view(self) -> ItemKind {
        let kind = self;
        match kind {
            rag_core::ItemKind::Chunk => ItemKind::Chunk,
            rag_core::ItemKind::Formula => ItemKind::Formula,
            rag_core::ItemKind::Figure => ItemKind::Figure,
            rag_core::ItemKind::Table => ItemKind::Table,
            rag_core::ItemKind::Code => ItemKind::Code,
        }
    }
}

impl IntoView<rag_core::ItemKind> for ItemKind {
    fn into_view(self) -> rag_core::ItemKind {
        let kind = self;
        match kind {
            ItemKind::Chunk => rag_core::ItemKind::Chunk,
            ItemKind::Formula => rag_core::ItemKind::Formula,
            ItemKind::Figure => rag_core::ItemKind::Figure,
            ItemKind::Table => rag_core::ItemKind::Table,
            ItemKind::Code => rag_core::ItemKind::Code,
        }
    }
}

impl IntoView<Category> for rag_core::Category {
    fn into_view(self) -> Category {
        let category = self;
        match category {
            rag_core::Category::Book => Category::Book,
            rag_core::Category::Paper => Category::Paper,
            rag_core::Category::Other => Category::Other,
        }
    }
}

impl IntoView<rag_core::Category> for Category {
    fn into_view(self) -> rag_core::Category {
        let category = self;
        match category {
            Category::Book => rag_core::Category::Book,
            Category::Paper => rag_core::Category::Paper,
            Category::Other => rag_core::Category::Other,
        }
    }
}

impl IntoView<DocumentName> for ocr::DocumentName {
    fn into_view(self) -> DocumentName {
        let name = self;
        match name {
            ocr::DocumentName::Chapter { number, name } => DocumentName::Chapter { number, name },
            ocr::DocumentName::Title(title) => DocumentName::Title(title),
        }
    }
}

impl IntoView<ocr::DocumentName> for &DocumentName {
    fn into_view(self) -> ocr::DocumentName {
        let name = self;
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

impl IntoView<LabelFilter> for &Filters {
    fn into_view(self) -> LabelFilter {
        let filters = self;
        LabelFilter {
            document: filters
                .document
                .map(|id| WantedDocument::Id(id.into_view())),
            media: filters.media.clone(),
            author: filters.author.clone(),
            category: filters.category.map(IntoView::into_view),
            tags: tags(&filters.tags).collect::<BTreeSet<_>>(),
        }
    }
}

/// A blank tag is dropped.
impl IntoView<TagChange> for &DocumentTagsEdit {
    fn into_view(self) -> TagChange {
        let edit = self;
        TagChange {
            add: tags(&edit.add).collect(),
            remove: tags(&edit.remove).collect(),
        }
    }
}

/// The title is trimmed, and a blank author or tag is dropped.
impl IntoView<MediaNode> for &NewMedia {
    fn into_view(self) -> MediaNode {
        let media = self;
        MediaNode {
            title: media.title.trim().to_owned(),
            labels: MediaLabels {
                category: media.category.into_view(),
                authors: author_list(&media.authors),
                tags: tags(&media.tags).collect(),
            },
        }
    }
}

/// An edit gives the whole new state, so every label is replaced.
impl IntoView<MediaChange> for &MediaEdit {
    fn into_view(self) -> MediaChange {
        let edit = self;
        MediaChange {
            category: Some(edit.category.into_view()),
            authors: Some(edit.authors.clone()),
            tags: Some(tags(&edit.tags).collect()),
        }
    }
}

impl IntoView<DatasetStage> for rag_dataset::Step {
    fn into_view(self) -> DatasetStage {
        let step = self;
        use rag_dataset::{GenerationKind, Step};
        let count = |number: usize| u32::try_from(number).unwrap_or(u32::MAX);
        match step {
            Step::Exporting { done, total } => DatasetStage::Exporting {
                done: count(done),
                total: count(total),
            },
            Step::Checking => DatasetStage::Checking,
            Step::Planning => DatasetStage::Planning,
            Step::Writing {
                kind: GenerationKind::Synthetic,
                done,
                total,
            } => DatasetStage::WritingSynthetic {
                done: count(done),
                total: count(total),
            },
            Step::Writing {
                kind: GenerationKind::Sft,
                done,
                total,
            } => DatasetStage::WritingSft {
                done: count(done),
                total: count(total),
            },
            Step::WritingManifest => DatasetStage::WritingManifest,
        }
    }
}

impl IntoView<Usage> for &UsageTally {
    fn into_view(self) -> Usage {
        let tally = self;
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

impl IntoView<InsightsOutcome> for &rag_ingestion::InsightsOutcome {
    fn into_view(self) -> InsightsOutcome {
        let outcome = self;
        match outcome {
            rag_ingestion::InsightsOutcome::Written { .. } => InsightsOutcome::Written,
            rag_ingestion::InsightsOutcome::AlreadySaved { .. } => InsightsOutcome::AlreadySaved,
            rag_ingestion::InsightsOutcome::NotWritten { reason } => InsightsOutcome::NotWritten {
                reason: reason.clone(),
            },
            rag_ingestion::InsightsOutcome::NoneForBookPart => InsightsOutcome::NoneForBookPart,
        }
    }
}
