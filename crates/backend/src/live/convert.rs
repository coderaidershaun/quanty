//! The ids, the item kind, the category, the document name, the label shapes, the steps of a build,
//! what a run used, how far a document is and what became of its insights, to and from the types
//! of the backend. Every part of the live backend converts through these and writes none of its
//! own.

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

macro_rules! ids {
    ($($id:ident),+) => {$(
        impl IntoView<$id> for rag_core::$id {
            fn into_view(self) -> $id {
                $id(self.0)
            }
        }

        impl IntoView<rag_core::$id> for $id {
            fn into_view(self) -> rag_core::$id {
                rag_core::$id(self.0)
            }
        }
    )+};
}

ids!(DocId, ItemId, ConceptId);

impl IntoView<PageConcept> for ConceptNode {
    fn into_view(self) -> PageConcept {
        PageConcept {
            id: self.id.into_view(),
            name: self.name,
            definition: self.definition,
        }
    }
}

impl IntoView<ItemKind> for rag_core::ItemKind {
    fn into_view(self) -> ItemKind {
        match self {
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
        match self {
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
        match self {
            rag_core::Category::Book => Category::Book,
            rag_core::Category::Paper => Category::Paper,
            rag_core::Category::Other => Category::Other,
        }
    }
}

impl IntoView<rag_core::Category> for Category {
    fn into_view(self) -> rag_core::Category {
        match self {
            Category::Book => rag_core::Category::Book,
            Category::Paper => rag_core::Category::Paper,
            Category::Other => rag_core::Category::Other,
        }
    }
}

impl IntoView<DocumentName> for ocr::DocumentName {
    fn into_view(self) -> DocumentName {
        match self {
            ocr::DocumentName::Chapter { number, name } => DocumentName::Chapter { number, name },
            ocr::DocumentName::Title(title) => DocumentName::Title(title),
        }
    }
}

impl IntoView<ocr::DocumentName> for &DocumentName {
    fn into_view(self) -> ocr::DocumentName {
        match self {
            DocumentName::Chapter { number, name } => ocr::DocumentName::Chapter {
                number: *number,
                name: name.clone(),
            },
            DocumentName::Title(title) => ocr::DocumentName::Title(title.clone()),
        }
    }
}

/// The tags that are not blank, in the form the backend stores them.
pub(super) fn tags(texts: &[String]) -> impl Iterator<Item = Tag> {
    texts.iter().filter_map(|text| text.parse().ok())
}

impl IntoView<LabelFilter> for &Filters {
    fn into_view(self) -> LabelFilter {
        LabelFilter {
            document: self.document.map(|id| WantedDocument::Id(id.into_view())),
            media: self.media.clone(),
            author: self.author.clone(),
            category: self.category.map(IntoView::into_view),
            tags: tags(&self.tags).collect::<BTreeSet<_>>(),
        }
    }
}

/// A blank tag is dropped.
impl IntoView<TagChange> for &DocumentTagsEdit {
    fn into_view(self) -> TagChange {
        TagChange {
            add: tags(&self.add).collect(),
            remove: tags(&self.remove).collect(),
        }
    }
}

/// The title is trimmed, and a blank author or tag is dropped.
impl IntoView<MediaNode> for &NewMedia {
    fn into_view(self) -> MediaNode {
        MediaNode {
            title: self.title.trim().to_owned(),
            labels: MediaLabels {
                category: self.category.into_view(),
                authors: author_list(&self.authors),
                tags: tags(&self.tags).collect(),
            },
        }
    }
}

/// An edit gives the whole new state, so every label is replaced.
impl IntoView<MediaChange> for &MediaEdit {
    fn into_view(self) -> MediaChange {
        MediaChange {
            category: Some(self.category.into_view()),
            authors: Some(self.authors.clone()),
            tags: Some(tags(&self.tags).collect()),
        }
    }
}

impl IntoView<DatasetStage> for rag_dataset::Step {
    fn into_view(self) -> DatasetStage {
        use rag_dataset::{GenerationKind, Step};
        let count = |number: usize| u32::try_from(number).unwrap_or(u32::MAX);
        match self {
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
        let models = self
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
            cost_usd: self.cost_usd(),
        }
    }
}

impl IntoView<contract::ChapterState> for rag_ingestion::DocumentState {
    fn into_view(self) -> contract::ChapterState {
        match self {
            rag_ingestion::DocumentState::New => contract::ChapterState::New,
            rag_ingestion::DocumentState::PartlyConverted { pages_done } => {
                contract::ChapterState::PartlyConverted { pages_done }
            }
            rag_ingestion::DocumentState::Converted => contract::ChapterState::Converted,
            rag_ingestion::DocumentState::Ingested { items } => {
                contract::ChapterState::Ingested { items }
            }
            rag_ingestion::DocumentState::IngestedWithoutMetadata {
                items,
                missing_artifacts,
                missing_insights,
            } => contract::ChapterState::IngestedWithoutMetadata {
                items,
                missing_artifacts,
                missing_insights,
            },
            rag_ingestion::DocumentState::IngestedWithoutInsights { items } => {
                contract::ChapterState::IngestedWithoutInsights { items }
            }
        }
    }
}

impl IntoView<InsightsOutcome> for &rag_ingestion::InsightsOutcome {
    fn into_view(self) -> InsightsOutcome {
        match self {
            rag_ingestion::InsightsOutcome::Written { .. } => InsightsOutcome::Written,
            rag_ingestion::InsightsOutcome::AlreadySaved { .. } => InsightsOutcome::AlreadySaved,
            rag_ingestion::InsightsOutcome::NotWritten { reason } => InsightsOutcome::NotWritten {
                reason: reason.clone(),
            },
            rag_ingestion::InsightsOutcome::NoneForBookPart => InsightsOutcome::NoneForBookPart,
        }
    }
}
