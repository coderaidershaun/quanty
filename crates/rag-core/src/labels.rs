//! The labels a document carries, and what a query may ask of them. A label belongs to the
//! document and never changes an identifier or the text that is embedded.

use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// One free label of a document, such as "options". It is kept in lower case with no space at
/// either end, so two spellings of a tag are one tag.
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

/// A label that is not there is left out of what is stored, so a document stored before labels
/// existed reads the same as a new one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocumentLabels {
    /// The title of the book as it was given when the chapter was converted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub book: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub tags: BTreeSet<Tag>,
}

impl DocumentLabels {
    pub fn is_empty(&self) -> bool {
        self.book.is_none() && self.author.is_none() && self.tags.is_empty()
    }

    /// Whether these labels have every label that `wanted` names. A book or an author matches
    /// whatever its capitals, and every wanted tag must be a tag of these labels. `wanted` that
    /// names nothing is carried by all labels.
    pub fn carries(&self, wanted: &DocumentLabels) -> bool {
        same_text(self.book.as_deref(), wanted.book.as_deref())
            && same_text(self.author.as_deref(), wanted.author.as_deref())
            && wanted.tags.is_subset(&self.tags)
    }
}

fn same_text(have: Option<&str>, wanted: Option<&str>) -> bool {
    wanted
        .is_none_or(|wanted| have.is_some_and(|have| have.to_lowercase() == wanted.to_lowercase()))
}
