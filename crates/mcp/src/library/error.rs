//! Everything that can stop a read of the library: an argument that is refused, a chapter that is
//! not there or cannot be read, and a store that is down.

use std::path::PathBuf;

use graph::GraphError;
use ocr::{ContentError, ReadChapterError};
use rag_core::{
    DocId, EmptyTag, ItemId, ParseDocIdError, StoreError, UnknownCategory, UnknownItemKind,
};
use rag_ingestion::{RelabelError, StoredChaptersError};

use crate::args::BudgetError;

#[derive(thiserror::Error, Debug)]
pub(crate) enum LibraryError {
    #[error("`document_id` is not valid; take it from a search result or from `list_documents`")]
    DocumentId(#[from] ParseDocIdError),

    #[error("`page` is 0; pages count from 1")]
    ZeroPage,

    #[error("`kind` is not valid")]
    ItemKind(#[source] UnknownItemKind),

    #[error("`media` is blank; give the title of a stored media, as `list_documents` gives it")]
    BlankMedia,

    #[error("nothing to change; give `category`, `authors` or `tags`")]
    NothingToChange,

    #[error("`category` is not valid")]
    Category(#[source] UnknownCategory),

    #[error(
        "a tag in `tags` is blank; give each tag as a word, or give `[]` to take every tag away"
    )]
    MediaTag(#[source] EmptyTag),

    #[error("a tag in `tags` is blank; give each tag as a word, or leave `tags` out")]
    FilterTag(#[source] EmptyTag),

    #[error("`limit` is {given}; give a whole number from 1 to {most}, or leave it out")]
    LimitOutOfRange { given: u32, most: u32 },

    #[error("nothing to change; give tags in `add` or `remove`")]
    NoTagChange,

    #[error("a tag in `add` or `remove` is blank; give each tag as a word")]
    DocumentTag(#[source] EmptyTag),

    #[error("the labels were not changed")]
    Relabel(#[source] Box<RelabelError>),

    #[error(
        "`chapter_contents` lists formulas, figures, tables and code listings; give one of `formula`, `figure`, `table` or `code`, or read the text of a page with `read_page`"
    )]
    ChunkKind,

    #[error("could not read the documents from the graph")]
    Graph(#[from] GraphError),

    #[error("could not set up the item store")]
    ItemStore(#[source] StoreError),

    #[error("could not set up the concept store")]
    ConceptStore(#[source] StoreError),

    #[error(
        "the graph holds no document with the id {id}; call `list_documents` to see the document ids"
    )]
    NotStored { id: DocId },

    #[error(
        "no stored media is titled {title:?}; call `list_documents` to see the titles of the media"
    )]
    UnknownMedia { title: String },

    #[error(
        "no stored document belongs to a media titled {title:?}; call `list_documents` to see the titles of the media"
    )]
    NoDocumentOfMedia { title: String },

    #[error(
        "could not list the converted chapters; check that the server can read its content folder"
    )]
    Content(#[from] ContentError),

    #[error(
        "no converted chapter under {} has the document id {id}; call `list_documents` to see the document ids",
        folder.display()
    )]
    UnknownDocument { id: DocId, folder: PathBuf },

    /// A chapter that cannot be read has no document id to compare, so the document that was
    /// asked for may be one of these.
    #[error(
        "no converted chapter that can be read under {} has the document id {id}; the document may be one of the chapters that cannot be read: {}; call `list_documents` to see the document ids",
        folder.display(),
        unreadable.join("; ")
    )]
    UnreadableChapters {
        id: DocId,
        folder: PathBuf,
        unreadable: Vec<String>,
    },

    #[error("could not read the converted chapter")]
    Chapter(#[from] ReadChapterError),

    #[error("page {page} is out of range: the chapter has {pages} pages, numbered from 1")]
    PageOutOfRange { page: u32, pages: u32 },

    #[error(transparent)]
    Budget(#[from] BudgetError),

    #[error("`from` is page {from} and `to` is page {to}; give `from` no later than `to`")]
    PagesReversed { from: u32, to: u32 },

    #[error(
        "section {index} is out of range: the document has {sections} sections, numbered from 1; call `document_outline` to see them"
    )]
    SectionOutOfRange { index: usize, sections: usize },

    #[error(
        "no section of the document has the heading {name:?}; call `document_outline` to see the sections and their `index`"
    )]
    NoSuchSection { name: String },

    #[error(
        "{name:?} is the heading of several sections, {indexes}; give one of these as a number"
    )]
    AmbiguousSection { name: String, indexes: String },

    #[error("give from 1 to {most} requests in `requests`, not {given}")]
    RequestCount { given: usize, most: usize },

    #[error("give the `document_id`, `kind` and `label` of the item, or its `item_id`")]
    ItemChoice,

    #[error(
        "`read_item` reads a formula, figure, table or code listing; read the text of a page with `read_page`"
    )]
    ItemKindIsText,

    #[error("`item_id` is not valid; take it from a result of `search`")]
    ItemId(#[source] uuid::Error),

    #[error(
        "the document has no {kind} labelled {label:?}; call `chapter_contents` to see its items and their labels"
    )]
    NoItemLabelled { kind: String, label: String },

    #[error(
        "no converted chapter holds the item {id}; call `search` to see the items and their ids"
    )]
    NoSuchItem { id: ItemId },

    #[error(
        "the item cannot be matched to the id it is stored under; read it with `chapter_contents` or `read_page`"
    )]
    ItemsDiffer,
}

/// Everything that stops a read of the insights saved for one document.
#[derive(thiserror::Error, Debug)]
pub(crate) enum SavedInsightsError {
    #[error(transparent)]
    Library(#[from] LibraryError),

    #[error(
        "could not read the saved insights of the document; call `write_insights` with this `document_id` to write them again, which is paid"
    )]
    Unreadable(#[source] ContentError),

    #[error(
        "could not read from the graph whether this document is a part of a book, which gets no insights"
    )]
    Category(#[source] GraphError),
}

impl SavedInsightsError {
    /// Whether the error says that the document asked for is not there, as opposed to an argument
    /// that is refused or a file or store that fails.
    pub(crate) fn is_not_found(&self) -> bool {
        matches!(self, SavedInsightsError::Library(error) if error.is_not_found())
    }

    /// Whether a store is the cause, so that the text of the failure points at `health`.
    pub(crate) fn is_service_failure(&self) -> bool {
        match self {
            SavedInsightsError::Library(error) => error.is_service_failure(),
            SavedInsightsError::Category(_) => true,
            SavedInsightsError::Unreadable(_) => false,
        }
    }
}

impl LibraryError {
    /// Whether the error says that nothing stored is the document, media, page or section asked
    /// for, as opposed to a refused argument or a store or file that fails. A document id that is
    /// not valid and page 0 name nothing either.
    pub(crate) fn is_not_found(&self) -> bool {
        matches!(
            self,
            LibraryError::DocumentId(_)
                | LibraryError::ZeroPage
                | LibraryError::NotStored { .. }
                | LibraryError::UnknownMedia { .. }
                | LibraryError::NoDocumentOfMedia { .. }
                | LibraryError::UnknownDocument { .. }
                | LibraryError::UnreadableChapters { .. }
                | LibraryError::PageOutOfRange { .. }
                | LibraryError::SectionOutOfRange { .. }
        )
    }

    /// Whether a store is the cause, so that the text of the failure points at `health`.
    pub(crate) fn is_service_failure(&self) -> bool {
        matches!(
            self,
            LibraryError::Graph(_)
                | LibraryError::ItemStore(_)
                | LibraryError::ConceptStore(_)
                | LibraryError::Relabel(_)
        )
    }
}

/// The words of a stored-chapters error are the command line's, so each is worded again here for
/// an agent, which sends arguments and not flags.
impl From<StoredChaptersError> for LibraryError {
    fn from(error: StoredChaptersError) -> LibraryError {
        match error {
            StoredChaptersError::Graph(source) => LibraryError::Graph(source),
            StoredChaptersError::Catalogue(source) => LibraryError::Content(source),
            StoredChaptersError::NotStored { id } => LibraryError::NotStored { id },
            StoredChaptersError::NoDocumentOfMedia { title } => {
                LibraryError::NoDocumentOfMedia { title }
            }
        }
    }
}

/// The words of an unknown document or media are the command line's, so each is worded again here
/// for an agent. A store that fails keeps its own words.
impl From<RelabelError> for LibraryError {
    fn from(error: RelabelError) -> LibraryError {
        match error {
            RelabelError::UnknownDocument { id } => LibraryError::NotStored { id },
            RelabelError::UnknownMedia { title } => LibraryError::UnknownMedia { title },
            failed @ (RelabelError::Graph(_) | RelabelError::Store(_)) => {
                LibraryError::Relabel(Box::new(failed))
            }
        }
    }
}
