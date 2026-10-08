//! The documents that are stored, grouped by book, the books a person saved before any chapter,
//! and the change a person can make to a document's labels.

use super::ids::DocId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ItemCounts {
    pub chunks: u64,
    pub formulas: u64,
    pub figures: u64,
    pub tables: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ChapterLabel {
    pub number: u32,
    pub name: String,
}

/// One stored document: a chapter, or a picture that stands alone.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Document {
    pub id: DocId,
    pub title: String,
    /// `None`: a lone picture, or the chapter's folder is gone.
    pub chapter: Option<ChapterLabel>,
    pub author: Option<String>,
    /// Each tag is lower case.
    pub tags: Vec<String>,
    pub pages: Option<u32>,
    pub items: ItemCounts,
    /// `None`: an ingest stopped or skipped an item, so the count is not known.
    pub ingested_items: Option<u64>,
    /// `None`: the source view cannot open it.
    pub folder: Option<std::path::PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Book {
    /// `None`: the documents have no book.
    pub title: Option<String>,
    /// What was saved with the book. A book that is only a label on documents has neither.
    pub author: Option<String>,
    /// Each tag is lower case.
    pub tags: Vec<String>,
    /// Empty for a book that was saved and has no chapter yet.
    pub chapters: Vec<Document>,
}

/// A book as a person saves it, before any chapter of it is added. The panel sends a title that is
/// not blank and has no space at its ends, and a backend still trims it, because a stored book
/// cannot be changed yet.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct NewBook {
    pub title: String,
    pub author: Option<String>,
    /// As the person typed them. The backend stores them lower case, each once.
    pub tags: Vec<String>,
}

/// Two titles name one book when they differ only in capitals and in space at the ends.
pub fn is_same_title(one: &str, other: &str) -> bool {
    one.trim().to_lowercase() == other.trim().to_lowercase()
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Catalogue {
    pub books: Vec<Book>,
}

impl Catalogue {
    /// The title, as the library has it, of the book that `title` names. Capitals and the space at
    /// the ends do not count.
    pub fn stored_title(&self, title: &str) -> Option<&str> {
        self.books
            .iter()
            .filter_map(|book| book.title.as_deref())
            .find(|stored| is_same_title(stored, title))
    }

    /// Puts the books in the order that every list shows them in: by title with no regard to
    /// capitals, then by the title as written, and the documents with no book last. The backends
    /// share it, so a saved book lands in the same place whichever one answers.
    pub fn sort_books(&mut self) {
        self.books.sort_by_cached_key(|book| {
            (
                book.title.is_none(),
                book.title.as_deref().map(str::to_lowercase),
                book.title.clone(),
            )
        });
    }

    pub fn documents(&self) -> impl Iterator<Item = &Document> {
        self.books.iter().flat_map(|book| book.chapters.iter())
    }

    pub fn document(&self, id: DocId) -> Option<&Document> {
        self.documents().find(|document| document.id == id)
    }

    pub fn book_of(&self, id: DocId) -> Option<&Book> {
        self.books
            .iter()
            .find(|book| book.chapters.iter().any(|document| document.id == id))
    }

    pub fn authors(&self) -> Vec<&str> {
        let mut authors: Vec<&str> = self
            .documents()
            .filter_map(|document| document.author.as_deref())
            .collect();
        authors.sort_unstable();
        authors.dedup();
        authors
    }

    pub fn tags(&self) -> Vec<&str> {
        let mut tags: Vec<&str> = self
            .documents()
            .flat_map(|document| document.tags.iter().map(String::as_str))
            .collect();
        tags.sort_unstable();
        tags.dedup();
        tags
    }
}

/// `author: None` leaves the author as it is, because the app offers no way to remove one. The
/// book is not here: it comes from the chapter.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct LabelEdit {
    pub doc: DocId,
    pub author: Option<String>,
    pub add: Vec<String>,
    pub remove: Vec<String>,
}

impl LabelEdit {
    /// The edit that takes `document` to this author and exactly these tags.
    pub fn toward(document: &Document, author: Option<&str>, tags: &[String]) -> LabelEdit {
        let mut wanted: Vec<String> = Vec::new();
        for tag in tags {
            let tag = tag.trim().to_lowercase();
            if !tag.is_empty() && !wanted.contains(&tag) {
                wanted.push(tag);
            }
        }
        let author = author
            .map(str::trim)
            .filter(|author| !author.is_empty())
            .filter(|author| document.author.as_deref() != Some(*author))
            .map(str::to_owned);
        LabelEdit {
            doc: document.id,
            author,
            add: wanted
                .iter()
                .filter(|tag| !document.tags.contains(tag))
                .cloned()
                .collect(),
            remove: document
                .tags
                .iter()
                .filter(|tag| !wanted.contains(tag))
                .cloned()
                .collect(),
        }
    }
}
