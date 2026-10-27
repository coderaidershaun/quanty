//! What is asked about a whole document and what comes back: the prompt and the schema that every
//! question carries, the text that is sent for a document, and the checks that a reply must pass.

use std::collections::BTreeSet;

use ocr::Insight;
use rag_core::{ItemKind, estimated_tokens};
use serde::Deserialize;
use serde_json::Value;

use crate::ingest::items::Item;

pub(crate) const SYSTEM_PROMPT: &str = include_str!("prompts/insights.md");
pub(super) const SCHEMA: &str = include_str!("prompts/insights.schema.json");

// Raise this after a change to the prompt, so that a saved file shows which wording wrote it.
pub(super) const PROMPT_VERSION: &str = "1";

/// More than a chapter or a paper of this library holds, with room over because four characters
/// to a token counts low on math.
const MAX_QUESTION_TOKENS: usize = 100_000;

const BLOCK_SEPARATOR: &str = "\n\n";

#[derive(thiserror::Error, Debug)]
pub(super) enum TextError {
    #[error("the chapter has no text")]
    NoText,

    #[error(
        "the text is about {tokens} tokens, which is more than the {max} tokens that one question may hold"
    )]
    TooLong { tokens: usize, max: usize },
}

/// The text of a whole document as it is stored, with a marker where each page starts and a line
/// where a section starts.
pub(super) struct DocumentText {
    pub text: String,
    /// The pages that a marker in the text names.
    pub pages: BTreeSet<u32>,
}

impl DocumentText {
    /// # Errors
    /// - [`TextError::NoText`] when there are no items
    /// - [`TextError::TooLong`] when the text is longer than one question may be
    pub(super) fn of(items: &[Item]) -> Result<DocumentText, TextError> {
        let first = items.first().ok_or(TextError::NoText)?;
        let doc_title = first.payload.doc_title.as_str();
        let mut blocks = vec![format!("Document: {doc_title}")];
        let mut pages = BTreeSet::new();
        let mut page = None;
        let mut section = None;
        for item in items {
            let payload = &item.payload;
            if page != Some(payload.page) {
                blocks.push(page_marker(item));
                pages.insert(payload.page);
                page = Some(payload.page);
            }
            let item_section = section_of(item, doc_title);
            if item_section != section {
                if let Some(headings) = item_section {
                    blocks.push(format!("Section: {headings}"));
                }
                section = item_section;
            }
            blocks.push(body_of(item));
        }
        let text = blocks.join(BLOCK_SEPARATOR);
        let tokens = estimated_tokens(&text);
        if tokens > MAX_QUESTION_TOKENS {
            return Err(TextError::TooLong {
                tokens,
                max: MAX_QUESTION_TOKENS,
            });
        }
        Ok(DocumentText { text, pages })
    }
}

fn page_marker(item: &Item) -> String {
    let payload = &item.payload;
    match &payload.printed_page {
        Some(printed) => format!("[page {}, printed {printed}]", payload.page),
        None => format!("[page {}]", payload.page),
    }
}

/// The headings an item sits under: its title without the document title in front. `None` for an
/// item that sits under no heading.
fn section_of<'a>(item: &'a Item, doc_title: &str) -> Option<&'a str> {
    let headings = item
        .input
        .title
        .strip_prefix(doc_title)?
        .strip_prefix(" > ")?;
    (!headings.is_empty()).then_some(headings)
}

/// What the item says, in the form the model reads best: a formula between `$$`, code fenced,
/// and a table or a figure as it is embedded.
fn body_of(item: &Item) -> String {
    let payload = &item.payload;
    let label = payload.label.as_deref();
    match payload.kind {
        ItemKind::Chunk => payload.text.clone(),
        ItemKind::Formula => match label {
            Some(label) => format!("$${}$$ {label}", payload.text),
            None => format!("$${}$$", payload.text),
        },
        ItemKind::Figure | ItemKind::Table => item.input.text.clone(),
        ItemKind::Code => {
            let language = payload.language.as_deref().unwrap_or_default();
            let listing = format!("```{language}\n{}\n```", payload.text);
            match label {
                Some(label) => format!("{label}\n{listing}"),
                None => listing,
            }
        }
    }
}

/// What a reply said, with the pages cut down to the ones the text shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Notes {
    pub insights: Vec<Insight>,
    pub takeaways: Vec<String>,
    pub cheat_sheet: String,
}

#[derive(thiserror::Error, Debug)]
pub(super) enum ReplyError {
    #[error("the reply does not have the shape that the schema asks for")]
    Shape(#[source] serde_json::Error),

    #[error("the reply has no {part}")]
    Empty { part: &'static str },
}

#[derive(Deserialize)]
struct Reply {
    insights: Vec<ReplyInsight>,
    takeaways: Vec<String>,
    cheat_sheet: String,
}

#[derive(Deserialize)]
struct ReplyInsight {
    text: String,
    pages: Vec<i64>,
}

impl Notes {
    /// A reply that does not fit is refused as a whole. Every text is trimmed, a blank insight or
    /// takeaway is dropped, and a page that no marker of the text names is dropped from the pages
    /// of an insight. The rest of the pages are sorted, with no page twice. A cheat sheet that was
    /// escaped once too often is unescaped.
    ///
    /// # Errors
    /// - [`ReplyError::Shape`] when a field is missing or has the wrong kind of value
    /// - [`ReplyError::Empty`] when no insight, no takeaway or no cheat sheet is left
    pub(super) fn from_value(value: &Value, shown: &BTreeSet<u32>) -> Result<Notes, ReplyError> {
        let reply = Reply::deserialize(value).map_err(ReplyError::Shape)?;
        let insights: Vec<Insight> = reply
            .insights
            .into_iter()
            .filter(|insight| !insight.text.trim().is_empty())
            .map(|insight| {
                let pages: BTreeSet<u32> = insight
                    .pages
                    .into_iter()
                    .filter_map(|page| u32::try_from(page).ok())
                    .filter(|page| shown.contains(page))
                    .collect();
                Insight {
                    text: insight.text.trim().to_owned(),
                    pages: pages.into_iter().collect(),
                }
            })
            .collect();
        let takeaways: Vec<String> = reply
            .takeaways
            .iter()
            .map(|takeaway| takeaway.trim().to_owned())
            .filter(|takeaway| !takeaway.is_empty())
            .collect();
        let cheat_sheet = cheat_sheet_of(&reply.cheat_sheet);
        for (part, is_empty) in [
            ("insight", insights.is_empty()),
            ("takeaway", takeaways.is_empty()),
            ("cheat sheet", cheat_sheet.is_empty()),
        ] {
            if is_empty {
                return Err(ReplyError::Empty { part });
            }
        }
        Ok(Notes {
            insights,
            takeaways,
            cheat_sheet,
        })
    }
}

/// The cheat sheet as Markdown. A model sometimes escapes it once too often, so that it comes as
/// one line with `\n` where each line ends and `\\` for each backslash of a formula. A cheat sheet
/// always has more than one section, so one with no line break that holds a `\n` is unescaped
/// once more. A model also sometimes ends it with a quote that opens nothing, and that is dropped.
fn cheat_sheet_of(reply: &str) -> String {
    let text = reply.trim();
    let mut text = if !text.contains('\n') && text.contains("\\n") {
        unescaped(text).trim().to_owned()
    } else {
        text.to_owned()
    };
    if text.ends_with('"') && text.matches('"').count() % 2 == 1 {
        text.pop();
        text.truncate(text.trim_end().len());
    }
    text
}

/// `\n`, `\"` and `\\` become a line break, a quote and a backslash. Every other backslash is
/// kept as it is, so a formula that was escaped only once keeps its commands.
fn unescaped(text: &str) -> String {
    let mut unescaped = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(char) = chars.next() {
        if char != '\\' {
            unescaped.push(char);
            continue;
        }
        match chars.next() {
            Some('n') => unescaped.push('\n'),
            Some('"') => unescaped.push('"'),
            Some('\\') => unescaped.push('\\'),
            Some(other) => {
                unescaped.push('\\');
                unescaped.push(other);
            }
            None => unescaped.push('\\'),
        }
    }
    unescaped
}
