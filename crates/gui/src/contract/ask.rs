//! A question as the person writes it, and what comes back: the results, how the search found
//! them, and the answer written from them.

use super::ids::{DocId, ItemId};
use super::source::ImageRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum AskMode {
    #[default]
    Answer,
    ResultsOnly,
}

/// A book or an author matches whatever its capitals; every tag must be on the document.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Filters {
    pub book: Option<String>,
    pub author: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct AskDraft {
    pub question: String,
    pub mode: AskMode,
    pub filters: Filters,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ItemKind {
    Chunk,
    Formula,
    Figure,
    Table,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Reason {
    Nearest,
    Concept(String),
    Cited { by: usize, label: String },
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct ResultItem {
    /// Its place among the hits before any were dropped, counting from 1. It is the citation
    /// number.
    pub number: usize,
    pub id: ItemId,
    pub kind: ItemKind,
    pub score: f32,
    pub reason: Reason,
    pub doc: DocId,
    pub doc_title: String,
    pub book: Option<String>,
    /// Its position in the chapter, counting from 1.
    pub page: u32,
    pub printed_page: Option<String>,
    pub label: Option<String>,
    /// Chunk Markdown, raw LaTeX, table Markdown, or a figure's explanation.
    pub text: String,
    pub image: Option<ImageRef>,
    /// The number of the piece. This and the next two are read from the chapter's folder on
    /// disk, and are `None` when the document has no folder or no piece on the page matches.
    pub piece: Option<u32>,
    pub caption: Option<String>,
    pub name: Option<String>,
}

/// `None` means the search did not reach that step.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct RetrievalTrace {
    /// `None` means no filter, and `Some(n)` means the filter matched n documents. With `Some(0)`
    /// the search stopped before it embedded the question, so the screen says "no document has
    /// these labels", not "no sources".
    pub documents_searched: Option<usize>,
    pub nearest: usize,
    /// Concept names, nearest and most mentioned first.
    pub seed_concepts: Option<Vec<String>>,
    /// Concept names one relation away.
    pub related_concepts: Option<Vec<String>>,
    /// The nearest items plus the items the graph added.
    pub candidates: Option<usize>,
    /// The candidates the store gave back.
    pub ranked: Option<usize>,
    /// The results left after the cap for each document.
    pub kept: Option<usize>,
    /// Document title and the number of items the cap dropped.
    pub passed_over: Vec<(String, usize)>,
    /// The labels pulled in. `None` when the search has a kind filter.
    pub cited: Option<Vec<String>>,
}

/// Why a search found nothing. A search has no score limit, so there is no other reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NothingFound {
    /// The search stopped before it looked at any item.
    NoDocumentHasTheLabels,
    NoItemMatchesTheFilters,
    LibraryHoldsNoItems,
}

impl NothingFound {
    /// What the person can do about it, in words that fit any place in the window.
    pub fn hint(self) -> &'static str {
        match self {
            NothingFound::NoDocumentHasTheLabels => {
                "Nothing was searched. Clear a filter in the Ask bar and ask again."
            }
            NothingFound::NoItemMatchesTheFilters => {
                "No item in your library matches the filters of this question. Clear a filter, then ask again."
            }
            NothingFound::LibraryHoldsNoItems => {
                "Your library holds no items yet. Add a chapter on the Ingest tab, then ask again."
            }
        }
    }
}

impl RetrievalTrace {
    /// Why a search with no result found nothing.
    pub fn why_nothing_was_found(&self) -> NothingFound {
        match self.documents_searched {
            Some(0) => NothingFound::NoDocumentHasTheLabels,
            Some(_) => NothingFound::NoItemMatchesTheFilters,
            None => NothingFound::LibraryHoldsNoItems,
        }
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Default)]
pub struct SearchReply {
    pub results: Vec<ResultItem>,
    pub trace: RetrievalTrace,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AnswerBlock {
    Heading(String),
    /// Text with inline `\( … \)` formulas, and the numbers of the results it cites.
    Paragraph {
        text: String,
        cites: Vec<usize>,
    },
    /// The card of result n: a formula, a figure or a table.
    Item(usize),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Answer {
    pub title: Option<String>,
    /// Empty means the stored items do not answer the question.
    pub blocks: Vec<AnswerBlock>,
    /// Standalone questions, also when `blocks` is empty.
    pub follow_ups: Vec<String>,
}
