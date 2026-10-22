//! The insights, takeaways and cheat sheet of one document, as they were saved in its chapter
//! folder when it was ingested or written for it later, and the words that go with them.

use super::catalogue::{Catalogue, Category, DocumentName};
use crate::failure::{Failure, FailureKind};
use crate::ids::DocId;

/// One insight: a short Markdown paragraph that may hold `$…$` formulas, and the pages it rests on.
#[derive(
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Default,
)]
pub struct Insight {
    pub text: String,
    /// Page positions, from 1.
    pub pages: Vec<u32>,
}

/// The insights, takeaways and cheat sheet saved for one document.
#[derive(
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Default,
)]
pub struct ChapterInsights {
    /// The model that wrote them, as its agent names it.
    pub model: String,
    pub insights: Vec<Insight>,
    /// One-line Markdown points.
    pub takeaways: Vec<String>,
    /// Markdown: short headed sections, formulas and small tables.
    pub cheat_sheet: String,
}

impl ChapterInsights {
    /// The command that writes the insights of `doc`, and writes them again over a file that
    /// cannot be read.
    pub fn command_for(doc: DocId) -> String {
        format!("rag-ingest insights --document {}", doc.0)
    }

    /// The saved insights of `doc` could not be read, and `detail` says why.
    pub fn unreadable(doc: DocId, detail: impl Into<String>) -> Failure {
        Failure::new(FailureKind::BadFile, detail).with_hint(format!(
            "The saved insights of this document cannot be read. Run {} to write them again; it asks the model, which is paid.",
            Self::command_for(doc)
        ))
    }
}

impl DocumentName {
    // SMELL: the converter keeps its own copy of this rule, and the contract may not name that
    // crate, so a change to one must be made in both.
    /// Whether a document of this name, in a media of this category, gets insights. A part of a
    /// book that is not a chapter, such as its index, gets none. A document whose category is not
    /// known counts as one of another media.
    pub fn gets_insights(&self, category: Option<Category>) -> bool {
        !matches!(
            (self, category),
            (DocumentName::Title(_), Some(Category::Book))
        )
    }
}

impl Catalogue {
    /// Whether the listed document `doc` gets insights, by the rule of its name and its media's
    /// category. The group with no media has no category. A document that is not listed counts as
    /// one that gets them, so the view reads it and says what it finds.
    pub fn gets_insights(&self, doc: DocId) -> bool {
        let (Some(document), Some(media)) = (self.document(doc), self.media_of(doc)) else {
            return true;
        };
        let category = media.title.is_some().then_some(media.category);
        document.name().gets_insights(category)
    }
}
