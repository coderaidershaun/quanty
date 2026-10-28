//! The tools that read the library and change its labels: the stored media and documents, the
//! outline, sections, pages and items of a chapter, its concepts and insights, and the labels of a
//! media or a document.

mod changed;
mod chapter;
mod concepts;
mod contents;
mod document;
mod documents;
mod error;
mod labels;
mod media;
mod page;
mod piece;

pub(crate) use changed::LibraryChanges;
pub(crate) use chapter::{DocumentHeader, find_chapter, parse_document_id};
pub(crate) use concepts::{ChapterConceptsArgs, ConceptView, ConceptsView, chapter_concepts};
pub(crate) use contents::{ChapterContentsArgs, ContentsView, chapter_contents};
pub(crate) use document::{
    ChapterInsightsArgs, DocumentOutlineArgs, DocumentOverviewArgs, InsightsView, ItemRead,
    ManyRead, Outline, Overview, PagesView, ReadItemArgs, ReadManyArgs, ReadPagesArgs,
    ReadSectionArgs, Saved, SectionRef, SectionView, chapter_insights, document_outline,
    document_overview, items_of_section, read_item, read_many, read_pages, read_section,
    saved_insights, sections_of_items,
};
pub(crate) use documents::{Documents, ListDocumentsArgs, list_documents};
pub(crate) use error::{LibraryError, SavedInsightsError};
pub(crate) use labels::{
    DocumentTagsView, EditDocumentTagsArgs, EditMediaArgs, MediaLabelsView, edit_document_tags,
    edit_media,
};
pub(crate) use media::{ListMediaArgs, Media, list_media};
pub(crate) use page::{PageView, ReadPageArgs, read_page};
