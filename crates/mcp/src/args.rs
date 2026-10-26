//! One rule for reading the text of a tool's arguments, so that every tool treats blank text and
//! blank tags the same way.

use std::path::Path;
use std::str::FromStr;

use rag_core::{EmptyTag, Tag};

/// The text with no space at either end, or `None` when nothing is left.
pub(crate) fn non_blank(text: Option<String>) -> Option<String> {
    text.map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

/// The trimmed text read as a `T`, or `None` when the text is blank.
pub(crate) fn parse_optional<T: FromStr>(text: Option<String>) -> Result<Option<T>, T::Err> {
    non_blank(text).map(|text| text.parse()).transpose()
}

/// The last part of the path as text, or `None` when the path has none.
pub(crate) fn file_name_of(path: impl AsRef<Path>) -> Option<String> {
    path.as_ref()
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

/// Every tag of the list, or the error for the first one that is blank.
pub(crate) fn parse_tags<C: FromIterator<Tag>>(texts: Option<Vec<String>>) -> Result<C, EmptyTag> {
    texts
        .unwrap_or_default()
        .iter()
        .map(|tag| tag.parse::<Tag>())
        .collect()
}
