//! What a search found, and how it prints: one block for each result, with the reason it is
//! there.

use std::fmt;

use rag_core::{ItemHit, ItemPayload};

/// Why an item is in the results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    Nearest,
    /// It came in through the graph: the name of the concept that led to it.
    Concept(String),
    /// A paragraph that is shown cites it by this printed label. `by` is the number, from 1, of
    /// that result.
    Cited {
        by: usize,
        label: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub item: ItemHit,
    pub reason: Reason,
}

/// The ranked items come first, then the items that they cite.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchResults {
    pub hits: Vec<SearchHit>,
}

impl fmt::Display for SearchResults {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.hits.is_empty() {
            return formatter.write_str("no items found");
        }
        for (index, hit) in self.hits.iter().enumerate() {
            if index > 0 {
                formatter.write_str("\n\n")?;
            }
            write_hit(formatter, index + 1, hit)?;
        }
        Ok(())
    }
}

pub(crate) fn page_text(payload: &ItemPayload) -> String {
    match &payload.printed_page {
        Some(printed) => printed.clone(),
        None => format!(
            "{} (position in the chapter, no printed number)",
            payload.page
        ),
    }
}

fn write_hit(formatter: &mut fmt::Formatter<'_>, number: usize, hit: &SearchHit) -> fmt::Result {
    let payload = &hit.item.payload;
    writeln!(formatter, "result {number}")?;
    writeln!(formatter, "document: {}", payload.doc_title)?;
    writeln!(formatter, "page: {}", page_text(payload))?;
    writeln!(formatter, "kind: {}", payload.kind.as_str())?;
    if let Some(label) = &payload.label {
        writeln!(formatter, "label: {label}")?;
    }
    writeln!(formatter, "score: {:.3}", hit.item.score)?;
    match &hit.reason {
        Reason::Nearest => {}
        Reason::Concept(name) => writeln!(formatter, "reached via concept {name}")?,
        Reason::Cited { by, label } => writeln!(formatter, "cited by result {by} as {label}")?,
    }
    if let Some(picture) = &payload.image_path {
        writeln!(formatter, "picture: {}", picture.display())?;
    }
    writeln!(formatter, "text:")?;
    formatter.write_str(&payload.text)
}
