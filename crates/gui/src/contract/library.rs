//! The documents that are stored, grouped by media, the media a person saved before any document,
//! and the changes a person can make to a media's labels and to a document's own tags.

use std::fmt;

use super::ids::DocId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ItemCounts {
    pub chunks: u64,
    pub formulas: u64,
    pub figures: u64,
    pub tables: u64,
}

impl fmt::Display for ItemCounts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn count(number: u64, noun: &str) -> String {
            match number {
                1 => format!("1 {noun}"),
                _ => format!("{number} {noun}s"),
            }
        }
        write!(
            formatter,
            "{}, {}, {} and {}",
            count(self.chunks, "passage"),
            count(self.formulas, "formula"),
            count(self.figures, "figure"),
            count(self.tables, "table")
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ChapterLabel {
    pub number: u32,
    pub name: String,
}

/// What kind of media a document belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum Category {
    #[default]
    Book,
    Paper,
    Other,
}

impl Category {
    pub const ALL: [Category; 3] = [Category::Book, Category::Paper, Category::Other];

    pub fn label(self) -> &'static str {
        match self {
            Category::Book => "Book",
            Category::Paper => "Paper",
            Category::Other => "Other",
        }
    }
}

/// How a document of a media is named: a chapter of a book, or a document with a title of its
/// own. The order puts chapters first, by number, and then titled documents by title.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DocumentName {
    Chapter { number: u32, name: String },
    Title(String),
}

impl DocumentName {
    /// Reads `chapter-<number>-<name>.pdf` the way the backend does. The contract may not name
    /// the crate that reads it for real, so the rule is written out here.
    // SMELL: the rule is written here and again in the crate that converts a chapter. A change to
    // one must be made in both.
    pub fn from_chapter_file_name(file_name: &str) -> Option<DocumentName> {
        let stem = file_name.strip_prefix("chapter-")?.strip_suffix(".pdf")?;
        let (digits, words) = stem.split_once('-')?;
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        let words: Vec<String> = words
            .split('-')
            .filter(|word| !word.is_empty())
            .map(|word| {
                let mut letters = word.chars();
                letters.next().map_or_else(String::new, |first| {
                    first.to_uppercase().chain(letters).collect()
                })
            })
            .collect();
        if words.is_empty() {
            return None;
        }
        Some(DocumentName::Chapter {
            number: digits.parse().ok()?,
            name: words.join(" "),
        })
    }

    /// "Chapter 3 · Greeks" for a chapter, and the title for a titled document.
    pub fn label(&self) -> String {
        match self {
            DocumentName::Chapter { number, name } => format!("Chapter {number} · {name}"),
            DocumentName::Title(title) => title.clone(),
        }
    }

    /// `None` for a titled document.
    pub fn chapter_label(&self) -> Option<ChapterLabel> {
        match self {
            DocumentName::Chapter { number, name } => Some(ChapterLabel {
                number: *number,
                name: name.clone(),
            }),
            DocumentName::Title(_) => None,
        }
    }

    /// `None` for a chapter.
    pub fn title(&self) -> Option<&str> {
        match self {
            DocumentName::Chapter { .. } => None,
            DocumentName::Title(title) => Some(title),
        }
    }
}

/// One stored document: a chapter, a document with a title of its own, or a picture that stands
/// alone.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Document {
    pub id: DocId,
    pub title: String,
    /// `None`: a titled document, a lone picture, or the chapter's folder is gone.
    pub chapter: Option<ChapterLabel>,
    /// Copied from the media, in its order.
    pub authors: Vec<String>,
    /// Copied from the media. Each tag is lower case.
    pub media_tags: Vec<String>,
    /// The document's own tags. Each tag is lower case.
    pub tags: Vec<String>,
    pub pages: Option<u32>,
    pub items: ItemCounts,
    /// `None`: an ingest stopped or skipped an item, so the count is not known.
    pub ingested_items: Option<u64>,
    /// `None`: the source view cannot open it.
    pub folder: Option<std::path::PathBuf>,
}

impl Document {
    /// The name the document is shown by: its chapter, or its title when it has no chapter.
    pub fn name(&self) -> DocumentName {
        match &self.chapter {
            Some(chapter) => DocumentName::Chapter {
                number: chapter.number,
                name: chapter.name.clone(),
            },
            None => DocumentName::Title(self.title.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Media {
    /// `None`: the documents that belong to no media, such as pictures that stand alone.
    pub title: Option<String>,
    pub category: Category,
    /// In the order they were given.
    pub authors: Vec<String>,
    /// The media tags. Each tag is lower case.
    pub tags: Vec<String>,
    /// Empty for a media that was saved and has no document yet.
    pub documents: Vec<Document>,
}

/// A media as a person saves it, before any document of it is added. The panel sends a title that
/// is not blank and has no space at its ends, and a backend still trims it, because the title of
/// a stored media cannot change.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct NewMedia {
    pub title: String,
    pub category: Category,
    /// As the person typed them. The backend trims them and drops blank ones.
    pub authors: Vec<String>,
    /// As the person typed them. The backend stores them lower case, each once.
    pub tags: Vec<String>,
}

/// The whole new state of a stored media's labels. The title names the media and cannot change.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct MediaEdit {
    pub title: String,
    pub category: Category,
    pub authors: Vec<String>,
    pub tags: Vec<String>,
}

/// A change to a document's own tags. The labels of its media are changed on the media.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct DocumentTagsEdit {
    pub doc: DocId,
    pub add: Vec<String>,
    pub remove: Vec<String>,
}

impl DocumentTagsEdit {
    pub fn is_empty(&self) -> bool {
        self.add.is_empty() && self.remove.is_empty()
    }

    /// The edit that takes `document` to exactly these own tags.
    pub fn toward(document: &Document, tags: &[String]) -> DocumentTagsEdit {
        let mut wanted: Vec<String> = Vec::new();
        for tag in tags {
            let tag = tag.trim().to_lowercase();
            if !tag.is_empty() && !wanted.contains(&tag) {
                wanted.push(tag);
            }
        }
        DocumentTagsEdit {
            doc: document.id,
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

// SMELL: the core crate writes this rule again as `rag_core::is_same_name`, and the contract may not
// name it. A change to one must be made in both.
pub fn is_same_title(one: &str, other: &str) -> bool {
    one.trim().to_lowercase() == other.trim().to_lowercase()
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Catalogue {
    pub media: Vec<Media>,
}

impl Catalogue {
    /// The title, as the library has it, of the media that `title` names. Capitals and the space
    /// at the ends do not count.
    pub fn stored_title(&self, title: &str) -> Option<&str> {
        self.media_titled(title)
            .and_then(|media| media.title.as_deref())
    }

    /// The media that `title` names, whatever its capitals and the space at its ends.
    pub fn media_titled(&self, title: &str) -> Option<&Media> {
        self.media.iter().find(|media| {
            media
                .title
                .as_deref()
                .is_some_and(|stored| is_same_title(stored, title))
        })
    }

    /// Puts the media in the order that every list shows them in: by title with no regard to
    /// capitals, then by the title as written, and the documents with no media last. The backends
    /// share it, so a saved media lands in the same place whichever one answers.
    pub fn sort_media(&mut self) {
        self.media.sort_by_cached_key(|media| {
            (
                media.title.is_none(),
                media.title.as_deref().map(str::to_lowercase),
                media.title.clone(),
            )
        });
    }

    pub fn documents(&self) -> impl Iterator<Item = &Document> {
        self.media.iter().flat_map(|media| media.documents.iter())
    }

    pub fn document(&self, id: DocId) -> Option<&Document> {
        self.documents().find(|document| document.id == id)
    }

    pub fn media_of(&self, id: DocId) -> Option<&Media> {
        self.media
            .iter()
            .find(|media| media.documents.iter().any(|document| document.id == id))
    }

    /// Every author of every document, each once, sorted.
    pub fn authors(&self) -> Vec<&str> {
        let mut authors: Vec<&str> = self
            .documents()
            .flat_map(|document| document.authors.iter().map(String::as_str))
            .collect();
        authors.sort_unstable();
        authors.dedup();
        authors
    }

    /// Every media tag and own tag of every document, each once, sorted.
    pub fn tags(&self) -> Vec<&str> {
        let mut tags: Vec<&str> = self
            .documents()
            .flat_map(|document| document.media_tags.iter().chain(&document.tags))
            .map(String::as_str)
            .collect();
        tags.sort_unstable();
        tags.dedup();
        tags
    }
}
