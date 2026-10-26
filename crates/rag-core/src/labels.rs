//! The labels of a media and of its documents, and what a query may ask of them. A label never
//! changes an identifier or the text that is embedded.

use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::DocId;

/// One free label of a media or of a document, such as "options". It is kept in lower case with
/// no space at either end, so two spellings of a tag are one tag.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct Tag(String);

impl Tag {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for Tag {
    type Err = EmptyTag;

    fn from_str(text: &str) -> Result<Tag, EmptyTag> {
        let tag = text.trim().to_lowercase();
        if tag.is_empty() {
            return Err(EmptyTag);
        }
        Ok(Tag(tag))
    }
}

// A tag that is read back from a store goes through the same rule as a tag that is typed.
impl TryFrom<String> for Tag {
    type Error = EmptyTag;

    fn try_from(text: String) -> Result<Tag, EmptyTag> {
        text.parse()
    }
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
#[error("a tag cannot be empty")]
pub struct EmptyTag;

/// What kind of media a document belongs to. It is stored in lower case: "book", "paper" or
/// "other".
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    #[default]
    Book,
    Paper,
    Other,
}

impl Category {
    pub const ALL: [Category; 3] = [Category::Book, Category::Paper, Category::Other];

    pub fn as_str(self) -> &'static str {
        match self {
            Category::Book => "book",
            Category::Paper => "paper",
            Category::Other => "other",
        }
    }
}

impl fmt::Display for Category {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(formatter)
    }
}

impl FromStr for Category {
    type Err = UnknownCategory;

    /// Reads a category whatever its capitals and the space at its ends.
    fn from_str(text: &str) -> Result<Category, UnknownCategory> {
        let wanted = text.trim().to_lowercase();
        Category::ALL
            .into_iter()
            .find(|category| category.as_str() == wanted)
            .ok_or_else(|| UnknownCategory {
                given: text.to_owned(),
            })
    }
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
#[error("`{given}` is not a category; the categories are book, paper and other")]
pub struct UnknownCategory {
    given: String,
}

/// The labels of a media. Every document of the media carries a copy of them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct MediaLabels {
    pub category: Category,
    /// In the order they were given.
    pub authors: Vec<String>,
    pub tags: BTreeSet<Tag>,
}

/// The one rule for a list of authors that a person typed: each name is trimmed, a blank one is
/// dropped, and of two equal names the first is kept. The order is kept.
pub fn author_list<S: AsRef<str>>(texts: impl IntoIterator<Item = S>) -> Vec<String> {
    let mut authors: Vec<String> = Vec::new();
    for text in texts {
        let author = text.as_ref().trim();
        if !author.is_empty() && !authors.iter().any(|kept| kept == author) {
            authors.push(author.to_owned());
        }
    }
    authors
}

/// Two titles of media, or two names of authors, are the same whatever their capitals and the
/// space at their ends.
// SMELL: the browser app keeps its own copy of this rule, and its contract may not name this
// crate, so a change to one must be made in both.
pub fn is_same_name(one: &str, other: &str) -> bool {
    one.trim().to_lowercase() == other.trim().to_lowercase()
}

/// The labels a document carries. A label that is not there is left out of what is stored.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocumentLabels {
    /// The title of the document's media. `None` for a picture that stands alone, which belongs to
    /// no media; such a document also has no category, no authors and no media tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<Category>,
    /// The authors of the media, in the order the media has them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub media_tags: BTreeSet<Tag>,
    /// The document's own tags.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub tags: BTreeSet<Tag>,
}

impl DocumentLabels {
    /// Makes these the labels of a document of that media: the media, its category, its authors
    /// and its tags are set together, and the document's own tags are left alone.
    pub fn take_media(&mut self, title: &str, media: &MediaLabels) {
        self.media = Some(title.to_owned());
        self.category = Some(media.category);
        self.authors = media.authors.clone();
        self.media_tags = media.tags.clone();
    }

    /// Whether the document is of the media with this title, whatever the capitals.
    pub fn is_of_media(&self, title: &str) -> bool {
        self.media
            .as_deref()
            .is_some_and(|media| is_same_name(media, title))
    }

    /// Whether these labels fit every label part that `wanted` gives; its document part is not
    /// looked at. A filter that gives no label part is carried by all labels.
    fn carries(&self, wanted: &LabelFilter) -> bool {
        let media_fits = wanted.media.as_deref().is_none_or(|media| {
            self.media
                .as_deref()
                .is_some_and(|have| is_same_name(have, media))
        });
        let author_fits = wanted
            .author
            .as_deref()
            .is_none_or(|author| self.authors.iter().any(|have| is_same_name(have, author)));
        let category_fits = wanted
            .category
            .is_none_or(|category| self.category == Some(category));
        let tags_fit = wanted
            .tags
            .iter()
            .all(|tag| self.media_tags.contains(tag) || self.tags.contains(tag));
        media_fits && author_fits && category_fits && tags_fit
    }
}

/// The one document that a search may be asked to look at alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WantedDocument {
    Id(DocId),
    /// Its whole stored title, whatever its capitals and the space at its ends, such as
    /// "Option Volatility and Pricing, chapter 1: Sample Pages" or, for a part of a book,
    /// "Option Volatility and Pricing, Index".
    Title(String),
}

impl From<&str> for WantedDocument {
    /// A text that is a document id names that id; any other text names a title.
    fn from(text: &str) -> WantedDocument {
        let text = text.trim();
        match text.parse() {
            Ok(id) => WantedDocument::Id(id),
            Err(_) => WantedDocument::Title(text.to_owned()),
        }
    }
}

/// What a search may ask of a document: the document itself, and its labels. Each part that is
/// given must fit.
// SMELL: the name says labels, but the filter also names one document. A rename would touch
// every crate that searches.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LabelFilter {
    /// One document, by its id or its whole stored title.
    pub document: Option<WantedDocument>,
    /// The title of the media, whatever its capitals.
    pub media: Option<String>,
    /// One author, whatever its capitals; it fits a document that has that author among others.
    pub author: Option<String>,
    pub category: Option<Category>,
    /// Each one must be a media tag or an own tag of the document.
    pub tags: BTreeSet<Tag>,
}

impl LabelFilter {
    pub fn is_empty(&self) -> bool {
        self.document.is_none()
            && self.media.is_none()
            && self.author.is_none()
            && self.category.is_none()
            && self.tags.is_empty()
    }

    /// Whether the document with this id, stored title and labels fits every part of the filter.
    pub fn fits(&self, id: DocId, title: &str, labels: &DocumentLabels) -> bool {
        let document_fits = self.document.as_ref().is_none_or(|wanted| match wanted {
            WantedDocument::Id(wanted_id) => *wanted_id == id,
            WantedDocument::Title(wanted_title) => is_same_name(title, wanted_title),
        });
        document_fits && labels.carries(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_document_fits_by_its_id_or_its_whole_stored_title_and_the_labels_must_fit_too() {
        let id = DocId::from_source_sha256("one");
        let other_id = DocId::from_source_sha256("two");
        let title = "Option Volatility & Pricing, Index";
        let labels = DocumentLabels::default();
        let fits = |text: &str| {
            LabelFilter {
                document: Some(WantedDocument::from(text)),
                ..LabelFilter::default()
            }
            .fits(id, title, &labels)
        };

        assert_eq!(
            WantedDocument::from(format!(" {id} ").as_str()),
            WantedDocument::Id(id)
        );
        assert_eq!(
            WantedDocument::from(" Some Title "),
            WantedDocument::Title("Some Title".to_owned())
        );
        assert!(fits(" option volatility & pricing, INDEX "));
        assert!(!fits("Option Volatility & Pricing"));
        assert!(!fits("Index"));
        assert!(fits(&id.to_string()));
        assert!(!fits(&other_id.to_string()));

        let with_a_tag_it_lacks = LabelFilter {
            document: Some(WantedDocument::Id(id)),
            tags: BTreeSet::from(["options".parse().unwrap()]),
            ..LabelFilter::default()
        };
        assert!(!with_a_tag_it_lacks.fits(id, title, &labels));
        assert!(LabelFilter::default().fits(id, title, &labels));
        assert!(
            !LabelFilter {
                document: Some(WantedDocument::Id(id)),
                ..LabelFilter::default()
            }
            .is_empty()
        );
    }
}
