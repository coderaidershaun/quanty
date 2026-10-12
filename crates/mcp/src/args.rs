//! One rule for reading the text of a tool's arguments, so that every tool treats blank text and
//! blank tags the same way.

use rag_core::{EmptyTag, Tag};

/// The text with no space at either end, or `None` when nothing is left.
pub(crate) fn non_blank(text: Option<String>) -> Option<String> {
    text.map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

/// Every tag of the list, or the error for the first one that is blank.
pub(crate) fn parse_tags<C: FromIterator<Tag>>(texts: Option<Vec<String>>) -> Result<C, EmptyTag> {
    texts
        .unwrap_or_default()
        .iter()
        .map(|tag| tag.parse::<Tag>())
        .collect()
}
