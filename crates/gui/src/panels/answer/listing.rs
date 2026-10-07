//! The counts and the words of every result row, made once for each search so that drawing
//! never builds a string.

use super::phase::AnswerTab;
use crate::contract::{ItemKind, Reason, ResultItem};

/// How many results the search found, in all and by kind. Text passages have no count of
/// their own: they are in `results` only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct Counts {
    pub(super) results: usize,
    pub(super) formulas: usize,
    pub(super) figures: usize,
    pub(super) tables: usize,
}

impl Counts {
    /// The count a tab shows. The Answer tab shows none.
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

/// The words of one result, in the place of the result in the search.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::sample;

    fn item(number: usize, kind: ItemKind) -> ResultItem {
        ResultItem {
            number,
            kind,
            doc_title: "Notes".to_owned(),
            page: 4,
            printed_page: Some("41".to_owned()),
            label: None,
            caption: None,
            reason: Reason::Nearest,
            score: 0.834,
            ..sample::search_reply().results.remove(0)
        }
    }

    #[test]
    fn the_rows_say_what_each_result_is_and_why_it_is_there() {
        let formula = ResultItem {
            label: Some("(2.4)".to_owned()),
            ..item(1, ItemKind::Formula)
        };
        let figure = ResultItem {
            label: Some("Figure 13-4".to_owned()),
            caption: Some("Three spreads.".to_owned()),
            printed_page: None,
            reason: Reason::Concept("Volatility".to_owned()),
            ..item(4, ItemKind::Figure)
        };
        let table = ResultItem {
            label: Some("Table 1-1".to_owned()),
            reason: Reason::Cited {
                by: 2,
                label: "Table 1-1".to_owned(),
            },
            ..item(5, ItemKind::Table)
        };
        let text = item(2, ItemKind::Chunk);
        let listing = Listing::of(&[formula, text, figure, table]);

        assert_eq!(
            listing.counts,
            Counts {
                results: 4,
                formulas: 1,
                figures: 1,
                tables: 1
            }
        );
        let row = |at: usize| &listing.rows[at];
        assert_eq!(row(0).kind_label, "Formula (2.4)");
        assert_eq!(row(1).kind_label, "Text");
        assert_eq!(row(2).kind_label, "Figure 13-4");
        assert_eq!(row(0).place, "Notes \u{b7} p. 41");
        assert_eq!(row(2).place, "Notes \u{b7} page 4");
        assert_eq!(row(0).reason, "Nearest to the question");
        assert_eq!(row(2).reason, "Reached through the concept Volatility");
        assert_eq!(row(3).reason, "Cited by result 2 as Table 1-1");
        assert_eq!(row(0).score, "0.83");
        assert_eq!(row(3).open_label, "Result 5");
        assert_eq!(row(2).caption, "**Figure 13-4** Three spreads.");
        assert_eq!(row(3).caption, "**Table 1-1**");
        assert_eq!(row(1).caption, "");
    }

    #[test]
    fn the_counts_of_a_tab_are_none_for_the_answer() {
        let counts = Listing::of(&[item(1, ItemKind::Chunk)]).counts;
        assert_eq!(counts.of(AnswerTab::Answer), None);
        assert_eq!(counts.of(AnswerTab::Results), Some(1));
        assert_eq!(counts.of(AnswerTab::Tables), Some(0));
    }
}
