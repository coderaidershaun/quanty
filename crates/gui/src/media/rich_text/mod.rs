//! Stored Markdown drawn as text that a person can read and copy: emphasis, code, formulas on
//! the baseline of their line, footnotes, citation chips and tables. A block is read and laid
//! out once, and kept until it is no longer drawn.

mod atom;
mod breaker;
mod flow;
mod layouts;
mod measure;
mod paint;
mod parse;
mod parse_table;
mod pieces;
mod place;
mod style;
mod table;

use eframe::egui;

use super::Media;
use crate::theme::TextRole;

pub use self::layouts::{LayoutStats, Layouts};

/// A block of Markdown with the numbers of the results it cites.
#[derive(Debug, Clone, Copy)]
pub struct RichText<'a> {
    pub markdown: &'a str,
    pub cites: &'a [usize],
    pub role: TextRole,
    /// The citation to draw as chosen.
    pub selected: Option<usize>,
}

impl<'a> RichText<'a> {
    /// A text with no citations, and no citation chosen.
    pub fn new(markdown: &'a str, role: TextRole) -> Self {
        RichText {
            markdown,
            cites: &[],
            role,
            selected: None,
        }
    }

    /// Sets the results that the text cites. Each gets a chip after the text.
    pub fn cites(mut self, cites: &'a [usize]) -> Self {
        self.cites = cites;
        self
    }

    /// Sets the citation whose chip is drawn as chosen.
    pub fn selected(mut self, cite: Option<usize>) -> Self {
        self.selected = cite;
        self
    }
}

/// What a click on the text asked for. Every copy in the app goes through
/// `Intent::CopyText`, so the text never reaches the clipboard from here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Clicked {
    /// A citation chip was clicked. It holds the number of the result.
    Citation(usize),
    /// "Copy text" or "Copy table" was chosen. It holds the stored source.
    CopyText(String),
}

/// Draws the text wrapped to the room that is left, with a chip after it for each citation.
/// Empty text draws nothing and takes no room. Returns the click, if there was one.
#[must_use = "a click or a copy request is lost"]
pub fn show(ui: &mut egui::Ui, media: &mut Media, text: &RichText<'_>) -> Option<Clicked> {
    if text.markdown.trim().is_empty() && text.cites.is_empty() {
        return None;
    }
    let flow = media.text.block(ui, &mut media.math, text);
    if flow.rows.is_empty() {
        return None;
    }
    paint::block(ui, &flow, text, &mut media.math)
}

/// Draws a Markdown table. Text that is not a table is drawn as `show` draws it. It only ever
/// returns `CopyText`.
#[must_use = "a copy request is lost"]
pub fn table(
    ui: &mut egui::Ui,
    media: &mut Media,
    markdown: &str,
    role: TextRole,
) -> Option<Clicked> {
    match media.text.table(ui, &mut media.math, markdown, role) {
        Some(layout) => table::show(ui, &mut media.math, &layout, markdown),
        None => show(ui, media, &RichText::new(markdown, role)),
    }
}
