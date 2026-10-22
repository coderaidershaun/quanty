//! The tools that read the library and change its labels: the stored documents, the pieces of one
//! page, the items and concepts of a chapter, and the labels of a media or a document.

mod chapter;
mod concepts;
mod contents;
mod documents;
mod error;
mod labels;
mod page;
mod piece;

pub(crate) use chapter::{DocumentHeader, find_chapter, parse_document_id};
pub(crate) use concepts::{ChapterConceptsArgs, ConceptsView, chapter_concepts};
pub(crate) use contents::{ChapterContentsArgs, ContentsView, chapter_contents};
pub(crate) use documents::{Documents, list_documents};
pub(crate) use error::LibraryError;
pub(crate) use labels::{
    DocumentTagsView, EditDocumentTagsArgs, EditMediaArgs, MediaLabelsView, edit_document_tags,
    edit_media,
};
pub(crate) use page::{PageView, ReadPageArgs, read_page};
