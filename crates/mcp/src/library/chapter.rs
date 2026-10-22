//! Finds the converted chapter that a document id names, and reads the facts of a chapter's name.

use std::path::Path;

use ocr::{Catalogue, ChapterEntry, DocumentName};
use rag_core::DocId;
use schemars::JsonSchema;
use serde::Serialize;

use super::LibraryError;

/// What names a document in the answer of a tool that reads one converted chapter.
#[derive(Serialize, JsonSchema)]
pub(crate) struct DocumentHeader {
    document_id: String,
    /// The title of the media the document belongs to.
    media: String,
    /// For a book chapter only.
    #[serde(skip_serializing_if = "Option::is_none")]
    chapter_number: Option<u32>,
    /// For a book chapter only.
    #[serde(skip_serializing_if = "Option::is_none")]
    chapter_name: Option<String>,
    /// For a document with a title of its own, such as a paper, only.
    #[serde(skip_serializing_if = "Option::is_none")]
    document_title: Option<String>,
    /// How many pages the chapter has.
    pages: u32,
}

impl DocumentHeader {
    pub(crate) fn of(entry: &ChapterEntry) -> DocumentHeader {
        let chapter = chapter_of(&entry.index.name);
        let document_title = match &entry.index.name {
            DocumentName::Title(title) => Some(title.clone()),
            DocumentName::Chapter { .. } => None,
        };
        DocumentHeader {
            document_id: document_of(entry).to_string(),
            media: entry.index.media_title.clone(),
            chapter_number: chapter.map(|(number, _)| number),
            chapter_name: chapter.map(|(_, name)| name.to_owned()),
            document_title,
            pages: entry.index.page_count,
        }
    }
}

/// The document id that an agent gave.
///
/// # Errors
/// [`LibraryError::DocumentId`] when the text is not a document id.
pub(crate) fn parse_document_id(text: &str) -> Result<DocId, LibraryError> {
    Ok(text.trim().parse()?)
}

/// The converted chapter that the document id names, found under the content folder.
///
/// # Errors
/// A content folder that cannot be listed, and a document that no converted chapter has or whose
/// chapter may be one that cannot be read.
pub(crate) fn find_chapter(content_folder: &Path, id: DocId) -> Result<ChapterEntry, LibraryError> {
    let catalogue = Catalogue::read(content_folder)?;
    chapter_entry(&catalogue, id, content_folder).cloned()
}

/// A chapter that cannot be read has no document id to compare, so when the catalogue has one,
/// the error says that it may be the document.
fn chapter_entry<'a>(
    catalogue: &'a Catalogue,
    id: DocId,
    content_folder: &Path,
) -> Result<&'a ChapterEntry, LibraryError> {
    catalogue
        .chapters
        .iter()
        .find(|entry| document_of(entry) == id)
        .ok_or_else(|| {
            let folder = content_folder.to_path_buf();
            if catalogue.unreadable.is_empty() {
                LibraryError::UnknownDocument { id, folder }
            } else {
                LibraryError::UnreadableChapters {
                    id,
                    folder,
                    unreadable: unreadable_chapters(catalogue),
                }
            }
        })
}

/// One line for each chapter of the catalogue that cannot be read. The line names the file.
pub(super) fn unreadable_chapters(catalogue: &Catalogue) -> Vec<String> {
    catalogue
        .unreadable
        .iter()
        .map(ToString::to_string)
        .collect()
}

/// The number and the name of a book chapter, and `None` for a document with a title of its own.
pub(super) fn chapter_of(name: &DocumentName) -> Option<(u32, &str)> {
    match name {
        DocumentName::Chapter { number, name } => Some((*number, name)),
        DocumentName::Title(_) => None,
    }
}

/// The document that a converted chapter is stored as: the same PDF always gives the same id.
pub(super) fn document_of(entry: &ChapterEntry) -> DocId {
    DocId::from_source_sha256(&entry.index.source_sha256)
}
