//! Which converted documents a build exports, what part of a work each one is, and which side of
//! the held-out line it falls on.

use std::path::PathBuf;

use ocr::content::file_name_text;
use ocr::{Catalogue, ChapterEntry, DocumentName};
use rag_core::{DocId, error_chain, is_same_name};
use rag_ingestion::document_title;
use serde::{Deserialize, Serialize};

use crate::options::{BuildOptions, Selection};

/// What part of a work a document is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocRole {
    /// A chapter of a book.
    Chapter,
    /// A part of a book that is not a chapter, such as its index or an appendix.
    Part,
    /// A whole work with no chapters, such as a paper.
    Document,
}

/// Which file of a dataset a document's sections go to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Split {
    Train,
    /// Kept out of training so that a model can be measured on text it has not seen.
    Heldout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDocument {
    pub id: DocId,
    /// The stored title, such as "Option Volatility and Pricing, chapter 1: Sample Pages".
    pub title: String,
    /// The title of the media the document belongs to.
    pub media: String,
    pub folder: PathBuf,
    pub role: DocRole,
    pub split: Split,
}

impl ExportDocument {
    /// `<media folder name>:<document folder name>`, the part of a sample id that tells which
    /// document it came from.
    pub fn slug(&self) -> String {
        let media_folder = self.folder.parent().map(file_name_text).unwrap_or_default();
        format!("{media_folder}:{}", file_name_text(&self.folder))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum SelectionError {
    #[error(
        "{entry:?} is not the title of any media in the content folder; give the title of a media, whatever its capitals"
    )]
    UnknownMedia { entry: String },

    #[error(
        "the documents to build name {entry:?}, which is no document in the content folder; give an id that an ingest prints as \"document id\", or the whole title of the document, such as \"Option Volatility and Pricing, chapter 1: Sample Pages\""
    )]
    UnknownDocument { entry: String },

    #[error(
        "the documents to hold out name {entry:?}, which is no document in the content folder; give an id that an ingest prints as \"document id\", or the whole title of the document, such as \"Option Volatility and Pricing, chapter 1: Sample Pages\""
    )]
    UnknownHoldout { entry: String },
}

/// A document of the catalogue, finished or not, with what a selection matches it by.
struct Candidate<'a> {
    entry: &'a ChapterEntry,
    id: DocId,
    title: String,
}

impl Candidate<'_> {
    fn is_named_by(&self, entry: &str) -> bool {
        entry.parse::<DocId>().is_ok_and(|id| id == self.id) || is_same_name(entry, &self.title)
    }

    fn is_in_media(&self, media: &str) -> bool {
        is_same_name(media, &self.entry.index.media_title)
    }
}

/// The finished documents the build exports, in the order of the catalogue, each with its role and
/// its side of the held-out line. A document that cannot be read is logged and left out.
///
/// # Errors
/// [`SelectionError`] when the selection names a media or a document that the catalogue does not
/// hold, finished or not. A misspelt held-out document would otherwise be trained on.
pub fn exported_documents(
    catalogue: &Catalogue,
    options: &BuildOptions,
) -> Result<Vec<ExportDocument>, SelectionError> {
    for error in &catalogue.unreadable {
        tracing::warn!(
            "a converted document was left out because it cannot be read: {}",
            error_chain(error)
        );
    }
    let candidates: Vec<Candidate> = catalogue
        .chapters
        .iter()
        .map(|entry| Candidate {
            entry,
            id: DocId::from_source_sha256(&entry.index.source_sha256),
            title: document_title(&entry.index),
        })
        .collect();
    let selection = &options.selection;
    check_selection(&candidates, selection)?;

    let books: Vec<&str> = candidates
        .iter()
        .filter(|candidate| matches!(candidate.entry.index.name, DocumentName::Chapter { .. }))
        .map(|candidate| candidate.entry.index.media_title.as_str())
        .collect();
    let exported = candidates
        .iter()
        .filter(|candidate| candidate.entry.index.finished)
        .filter(|candidate| is_selected(candidate, selection))
        .filter_map(|candidate| {
            let role = role_of(candidate, &books);
            let is_wanted_part = options.include_book_parts || has_appendix_title(candidate);
            (role != DocRole::Part || is_wanted_part).then(|| ExportDocument {
                id: candidate.id,
                title: candidate.title.clone(),
                media: candidate.entry.index.media_title.clone(),
                folder: candidate.entry.folder.clone(),
                role,
                split: split_of(candidate, selection),
            })
        })
        .collect();
    Ok(exported)
}

fn check_selection(candidates: &[Candidate], selection: &Selection) -> Result<(), SelectionError> {
    for entry in &selection.media {
        if !candidates
            .iter()
            .any(|candidate| candidate.is_in_media(entry))
        {
            return Err(SelectionError::UnknownMedia {
                entry: entry.clone(),
            });
        }
    }
    let names_a_document = |entry: &str| {
        candidates
            .iter()
            .any(|candidate| candidate.is_named_by(entry))
    };
    if let Some(entry) = selection
        .documents
        .iter()
        .find(|entry| !names_a_document(entry))
    {
        return Err(SelectionError::UnknownDocument {
            entry: entry.clone(),
        });
    }
    if let Some(entry) = selection
        .holdout
        .iter()
        .find(|entry| !names_a_document(entry))
    {
        return Err(SelectionError::UnknownHoldout {
            entry: entry.clone(),
        });
    }
    Ok(())
}

fn is_selected(candidate: &Candidate, selection: &Selection) -> bool {
    let in_chosen_media = selection.media.is_empty()
        || selection
            .media
            .iter()
            .any(|media| candidate.is_in_media(media));
    let is_chosen_document = selection.documents.is_empty()
        || selection
            .documents
            .iter()
            .any(|entry| candidate.is_named_by(entry));
    in_chosen_media && is_chosen_document
}

/// A titled document of a media that has chapters is a part of that book. A titled document of a
/// media without chapters is the whole work, such as a paper.
fn role_of(candidate: &Candidate, books: &[&str]) -> DocRole {
    match candidate.entry.index.name {
        DocumentName::Chapter { .. } => DocRole::Chapter,
        DocumentName::Title(_) if books.iter().any(|book| candidate.is_in_media(book)) => {
            DocRole::Part
        }
        DocumentName::Title(_) => DocRole::Document,
    }
}

/// An appendix holds worked material, so it is exported even when the other parts of a book are
/// not.
fn has_appendix_title(candidate: &Candidate) -> bool {
    candidate.title.to_lowercase().contains("appendix")
}

fn split_of(candidate: &Candidate, selection: &Selection) -> Split {
    if selection
        .holdout
        .iter()
        .any(|entry| candidate.is_named_by(entry))
    {
        Split::Heldout
    } else {
        Split::Train
    }
}
