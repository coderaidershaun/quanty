//! What the Library shows of the stored documents: how they are grouped and labelled, the items
//! of a chapter read from its folder, and its saved insights.

mod catalogue;
mod contents;
mod insights;

pub(crate) use catalogue::count;
pub use catalogue::{
    Catalogue, Category, ChapterLabel, Document, DocumentName, DocumentTagsEdit, DocumentView,
    ItemCounts, Media, MediaEdit, NewMedia, is_same_name,
};
pub(crate) use contents::page_words;
pub use contents::{ArtifactMetadata, ChapterContents, ChapterItem};
pub use insights::{ChapterInsights, Insight};
