//! The one way the Markdown of a document is read, so that the three files agree on what a
//! paragraph, a table, a formula and a footnote are.

use pulldown_cmark::{Event, Options, Parser, TagEnd};

pub(crate) fn parse(markdown: &str) -> Parser<'_> {
    Parser::new_ext(
        markdown,
        Options::ENABLE_TABLES | Options::ENABLE_MATH | Options::ENABLE_FOOTNOTES,
    )
}

/// Whether the last thing in `markdown` is a paragraph, so that words written after it are read
/// as the end of that paragraph.
pub(crate) fn ends_with_paragraph(markdown: &str) -> bool {
    matches!(parse(markdown).last(), Some(Event::End(TagEnd::Paragraph)))
}
