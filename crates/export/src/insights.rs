//! The insights, takeaways and cheat sheet of one document as a document that goes in a file, so
//! the browser app and the MCP server write the same file from the same values.

use crate::{Block, Document};

/// What was saved for one document, in plain values.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct InsightsSheet {
    /// The whole stored title, as `list_documents` gives it.
    pub document_title: String,
    /// The model that wrote them, from the saved file.
    pub model: String,
    /// In the order they were saved.
    pub insights: Vec<SheetInsight>,
    /// One line of Markdown each.
    pub takeaways: Vec<String>,
    /// Markdown: `###` sections, `$…$` and `$$…$$` formulas, small GitHub tables.
    pub cheat_sheet: String,
}

/// One insight and the pages it rests on.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct SheetInsight {
    /// A short Markdown paragraph that may hold `$…$` and `$$…$$`.
    pub text: String,
    /// Page places from 1.
    pub pages: Vec<u32>,
}

/// The title, a line that names the model, then Insights (each with its pages), Takeaways (a
/// list) and the Cheat sheet (its sections, formulas and tables). A part with nothing in it is
/// left out.
pub fn insights_document(sheet: &InsightsSheet) -> Document {
    let mut blocks = Vec::new();
    if !sheet.insights.is_empty() {
        blocks.push(Block::Heading("Insights".to_owned()));
        blocks.extend(sheet.insights.iter().map(|insight| Block::Text {
            markdown: insight_markdown(insight),
            cites: Vec::new(),
        }));
    }
    if !sheet.takeaways.is_empty() {
        blocks.push(Block::Heading("Takeaways".to_owned()));
        blocks.push(Block::List(sheet.takeaways.clone()));
    }
    if !sheet.cheat_sheet.trim().is_empty() {
        blocks.push(Block::Heading("Cheat sheet".to_owned()));
        blocks.push(Block::Text {
            markdown: sheet.cheat_sheet.clone(),
            cites: Vec::new(),
        });
    }
    Document {
        title: sheet.document_title.clone(),
        origin: format!(
            "Insights, takeaways and cheat sheet written by {}",
            sheet.model
        ),
        blocks,
        sources: Vec::new(),
    }
}

/// The text of an insight, then its pages as a paragraph of their own, so that a formula set
/// apart at the end of the text is not broken by them.
fn insight_markdown(insight: &SheetInsight) -> String {
    match pages_line(&insight.pages) {
        Some(pages) => format!("{}\n\n{pages}", insight.text.trim_end()),
        None => insight.text.clone(),
    }
}

/// `*Page 3*`, `*Pages 1 and 3*` or `*Pages 1, 2 and 4*`; `None` when there is no page.
fn pages_line(pages: &[u32]) -> Option<String> {
    let numbers: Vec<String> = pages.iter().map(u32::to_string).collect();
    match numbers.as_slice() {
        [] => None,
        [only] => Some(format!("*Page {only}*")),
        [first @ .., last] => Some(format!("*Pages {} and {last}*", first.join(", "))),
    }
}
