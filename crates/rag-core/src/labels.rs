//! The labels of a media and of its documents, and what a query may ask of them. A label never
//! changes an identifier or the text that is embedded.

use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

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

    /// Whether these labels fit every part that `wanted` gives. A filter that gives nothing is
    /// carried by all labels.
    pub fn carries(&self, wanted: &LabelFilter) -> bool {
        let media_fits = wanted.media.as_deref().is_none_or(|media| {
            self.media
                .as_deref()
                .is_some_and(|have| same_text(have, media))
        });
        let author_fits = wanted
            .author
            .as_deref()
            .is_none_or(|author| self.authors.iter().any(|have| same_text(have, author)));
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

fn same_text(have: &str, wanted: &str) -> bool {
    have.to_lowercase() == wanted.to_lowercase()
}

/// What a search may ask of a document's labels. Each part that is given must fit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LabelFilter {
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
        self.media.is_none()
            && self.author.is_none()
            && self.category.is_none()
            && self.tags.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authors_are_trimmed_kept_once_and_in_order_and_a_category_reads_any_capitals() {
        assert_eq!(author_list([" B ", "", "A", "B"]), vec!["B", "A"]);
        assert_eq!(" Paper ".parse(), Ok(Category::Paper));
        assert!("journal".parse::<Category>().is_err());
    }
}
