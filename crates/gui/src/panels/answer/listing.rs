//! The counts and the words of every result row, made once for each search so that drawing
//! never builds a string.

use super::phase::AnswerTab;
use crate::contract::{ItemKind, Reason, ResultItem};

/// Text passages have no count of their own: they are in `results` only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct Counts {
    pub(super) results: usize,
    pub(super) formulas: usize,
    pub(super) figures: usize,
    pub(super) tables: usize,
}

impl Counts {
    pub(super) const fn of(self, tab: AnswerTab) -> Option<usize> {
        match tab {
            AnswerTab::Answer => None,
            AnswerTab::Results => Some(self.results),
            AnswerTab::Formulas => Some(self.formulas),
            AnswerTab::Figures => Some(self.figures),
            AnswerTab::Tables => Some(self.tables),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct Row {
    pub(super) kind_label: String,
    pub(super) place: String,
    pub(super) reason: String,
    pub(super) score: String,
    pub(super) open_label: String,
    /// `**{label}** {caption}`, either part left out when the chapter does not have it.
    pub(super) caption: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct Listing {
    pub(super) counts: Counts,
    pub(super) rows: Vec<Row>,
}

impl Listing {
    pub(super) fn of(results: &[ResultItem]) -> Listing {
        let count = |kind| results.iter().filter(|item| item.kind == kind).count();
        Listing {
            counts: Counts {
                results: results.len(),
                formulas: count(ItemKind::Formula),
                figures: count(ItemKind::Figure),
                tables: count(ItemKind::Table),
            },
            rows: results.iter().map(Row::of).collect(),
        }
    }
}

impl Row {
    fn of(item: &ResultItem) -> Row {
        Row {
            kind_label: kind_label(item),
            place: place(item),
            reason: reason(&item.reason),
            score: format!("{:.2}", item.score),
            open_label: format!("Result {}", item.number),
            caption: caption(item),
        }
    }
}

/// The kind's name, and the printed label when the label adds to it: "Figure 13-4" already
/// names its kind, "(2.4)" does not.
fn kind_label(item: &ResultItem) -> String {
    let name = match item.kind {
        ItemKind::Chunk => "Text",
        ItemKind::Formula => "Formula",
        ItemKind::Figure => "Figure",
        ItemKind::Table => "Table",
    };
    match item.label.as_deref() {
        Some(label) if label.starts_with(name) => label.to_owned(),
        Some(label) => format!("{name} {label}"),
        None => name.to_owned(),
    }
}

fn place(item: &ResultItem) -> String {
    match &item.printed_page {
        Some(printed) => format!("{} \u{b7} p. {printed}", item.doc_title),
        None => format!("{} \u{b7} page {}", item.doc_title, item.page),
    }
}

fn reason(reason: &Reason) -> String {
    match reason {
        Reason::Nearest => "Nearest to the question".to_owned(),
        Reason::Concept(name) => format!("Reached through the concept {name}"),
        Reason::Cited { by, label } => format!("Cited by result {by} as {label}"),
    }
}

/// Only a figure and a table have a caption line under their card.
fn caption(item: &ResultItem) -> String {
    if !matches!(item.kind, ItemKind::Figure | ItemKind::Table) {
        return String::new();
    }
    match (item.label.as_deref(), item.caption.as_deref()) {
        (Some(label), Some(caption)) => format!("**{label}** {caption}"),
        (Some(label), None) => format!("**{label}**"),
        (None, Some(caption)) => caption.to_owned(),
        (None, None) => String::new(),
    }
}
