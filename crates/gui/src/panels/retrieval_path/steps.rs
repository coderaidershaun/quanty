//! Every word of the path and the rule that picks it: what the panel says when it has no steps
//! to show, and the five steps themselves.

use crate::contract::{Failure, Loadable, RetrievalTrace};
use crate::state::AskSession;
use crate::theme::{Icon, Kind, Tone};

pub(super) const TITLE: &str = "Retrieval Path";
pub(super) const SEARCHING: &str = "Searching";
pub(super) const FAILED: &str = "The search failed";

/// What fills the card under the header.
#[derive(Debug)]
pub(super) enum Body<'a> {
    Steps,
    Empty {
        icon: Icon,
        title: &'static str,
        hint: &'static str,
    },
    Failed(&'a Failure),
}

pub(super) fn body(ask: &AskSession) -> Body<'_> {
    match &ask.search {
        Loadable::Loading | Loadable::Ready(_) => Body::Steps,
        Loadable::Failed(failure) => Body::Failed(failure),
        Loadable::Idle if ask.question.is_empty() => Body::Empty {
            icon: Icon::SEARCH,
            title: "No search yet",
            hint: "Ask a question to see how its results are found.",
        },
        Loadable::Idle => Body::Empty {
            icon: Icon::STOP,
            title: "Search stopped",
            hint: "Ask again to see how the results are found.",
        },
    }
}

/// One row of the path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Step {
    pub(super) title: &'static str,
    pub(super) line: String,
    pub(super) badge: Option<String>,
    pub(super) progress: Progress,
    pub(super) tone: Tone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Progress {
    Waiting,
    Taken,
    NotReached,
}

/// The five rows of a search that has not come back.
pub(super) fn waiting() -> Vec<Step> {
    ROWS.iter()
        .map(|row| {
            row.step(Outcome {
                progress: Progress::Waiting,
                badge: None,
                line: None,
            })
        })
        .collect()
}

/// The five rows of a search that came back, from what its trace says each step did.
pub(super) fn taken(trace: &RetrievalTrace) -> Vec<Step> {
    let outcomes = [
        nearest(trace),
        concepts(trace),
        hop(trace),
        ranked(trace),
        cited(trace),
    ];
    ROWS.iter()
        .zip(outcomes)
        .map(|(row, outcome)| row.step(outcome))
        .collect()
}

const NAMES_SHOWN: usize = 3;
const NOT_REACHED: &str = "Not reached";

fn nearest(trace: &RetrievalTrace) -> Outcome {
    let items = counted(trace.nearest, "item", "items");
    let (badge, line) = match (trace.documents_searched, trace.nearest) {
        (Some(0), _) => (
            counted(0, "document", "documents"),
            Some("No document has these labels. Nothing was searched.".to_owned()),
        ),
        (Some(_), 0) => (items, Some("No item matches the filters.".to_owned())),
        (None, 0) => (items, Some("The library holds no items.".to_owned())),
        (Some(documents), _) => (
            items,
            Some(format!(
                "The closest items in the {} with these labels.",
                counted(documents, "document", "documents")
            )),
        ),
        (None, _) => (items, None),
    };
    Outcome {
        progress: Progress::Taken,
        badge: Some(badge),
        line,
    }
}

fn concepts(trace: &RetrievalTrace) -> Outcome {
    let Some(names) = &trace.seed_concepts else {
        return Outcome::not_reached();
    };
    let line = if names.is_empty() {
        "No concept in the graph is near the question or its items.".to_owned()
    } else {
        listed(names)
    };
    Outcome {
        progress: Progress::Taken,
        badge: Some(counted(names.len(), "concept", "concepts")),
        line: Some(line),
    }
}

fn hop(trace: &RetrievalTrace) -> Outcome {
    if trace.related_concepts.is_none() && trace.candidates.is_none() {
        return Outcome::not_reached();
    }
    let badge = trace
        .candidates
        .map(|candidates| added(candidates.saturating_sub(trace.nearest)));
    let line = trace.related_concepts.as_deref().map(|names| {
        if names.is_empty() {
            "No related concept in the graph.".to_owned()
        } else {
            listed(names)
        }
    });
    Outcome {
        progress: Progress::Taken,
        badge,
        line,
    }
}

fn ranked(trace: &RetrievalTrace) -> Outcome {
    if trace.ranked.is_none() && trace.kept.is_none() {
        return Outcome::not_reached();
    }
    let ranking = trace.ranked.map(|ranked| ranking(trace, ranked));
    let cap = trace.kept.map(|_| cap(&trace.passed_over));
    let line = match (ranking, cap) {
        (Some(ranking), Some(cap)) => Some(format!("{ranking} {cap}")),
        (ranking, cap) => ranking.or(cap),
    };
    Outcome {
        progress: Progress::Taken,
        badge: trace.kept.map(|kept| format!("{kept} kept")),
        line,
    }
}

/// A filter keeps the graph's items of other documents out of the ranking, so the ranked count
/// can be below the candidate count only when there was a filter.
fn ranking(trace: &RetrievalTrace, ranked: usize) -> String {
    match trace.candidates {
        Some(candidates) if trace.documents_searched.is_some() && ranked < candidates => {
            format!("{ranked} of {candidates} ranked together; the others lack these labels.")
        }
        _ => format!("{ranked} ranked together."),
    }
}

fn cap(passed_over: &[(String, usize)]) -> String {
    match passed_over {
        [] => "The cap passed over none.".to_owned(),
        [(title, dropped)] => format!("The cap passed over {dropped}: {title}."),
        several => {
            let total: usize = several.iter().map(|(_, dropped)| dropped).sum();
            let parts: Vec<String> = several
                .iter()
                .map(|(title, dropped)| format!("{dropped} from {title}"))
                .collect();
            format!("The cap passed over {total}: {}.", parts.join("; "))
        }
    }
}

fn cited(trace: &RetrievalTrace) -> Outcome {
    let Some(labels) = &trace.cited else {
        return Outcome::not_reached();
    };
    let line = if labels.is_empty() {
        "No result cites an item that is not already found.".to_owned()
    } else {
        listed(labels)
    };
    Outcome {
        progress: Progress::Taken,
        badge: Some(added(labels.len())),
        line: Some(line),
    }
}

/// `+3 items`: what a step added to the items already found.
fn added(count: usize) -> String {
    format!("+{}", counted(count, "item", "items"))
}

fn counted(count: usize, one: &str, many: &str) -> String {
    let noun = if count == 1 { one } else { many };
    format!("{count} {noun}")
}

/// The first few names, then how many more there are.
fn listed(names: &[String]) -> String {
    let shown: Vec<&str> = names.iter().take(NAMES_SHOWN).map(String::as_str).collect();
    let shown = shown.join(", ");
    match names.len().saturating_sub(NAMES_SHOWN) {
        0 => shown,
        more => format!("{shown} and {more} more"),
    }
}

/// A row of the path as it reads before it knows what its step did. Steps 4 and 5 of the
/// search share one row, because the backend runs them as one.
struct Row {
    title: &'static str,
    about: &'static str,
    tone: Tone,
}

const ROWS: [Row; 5] = [
    Row {
        title: "Nearest items",
        about: "The stored items closest in meaning to the question.",
        tone: Tone::Blue,
    },
    Row {
        title: "Concepts matched",
        about: "Concepts near the question or mentioned in those items.",
        tone: Kind::Concept.tone(),
    },
    Row {
        title: "One hop in the graph",
        about: "Related concepts, and the items that mention any concept.",
        tone: Kind::RelatedConcept.tone(),
    },
    Row {
        title: "Ranked and capped",
        about: "One ranking of all candidates, then a cap per document.",
        tone: Tone::Blue,
    },
    Row {
        title: "Cited items",
        about: "Figures, tables and equations that a result cites.",
        tone: Kind::Figure.tone(),
    },
];

/// What one step did. A line of `None` leaves the row's own about line.
struct Outcome {
    progress: Progress,
    badge: Option<String>,
    line: Option<String>,
}

impl Outcome {
    /// The search stopped before this step: the badge says so and the about line stays.
    fn not_reached() -> Self {
        Outcome {
            progress: Progress::NotReached,
            badge: Some(NOT_REACHED.to_owned()),
            line: None,
        }
    }
}

impl Row {
    fn step(&self, outcome: Outcome) -> Step {
        Step {
            title: self.title,
            line: outcome.line.unwrap_or_else(|| self.about.to_owned()),
            badge: outcome.badge,
            progress: outcome.progress,
            tone: self.tone,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_search_that_has_not_come_back_waits_on_five_rows_with_no_count() {
        let rows = waiting();
        let titles: Vec<_> = rows.iter().map(|row| row.title).collect();
        assert_eq!(
            titles,
            [
                "Nearest items",
                "Concepts matched",
                "One hop in the graph",
                "Ranked and capped",
                "Cited items"
            ]
        );
        assert!(rows.iter().all(|row| row.badge.is_none()));
        assert!(rows.iter().all(|row| row.progress == Progress::Waiting));
        assert_eq!(
            rows[0].line,
            "The stored items closest in meaning to the question."
        );
    }

    fn strings(names: &[&str]) -> Option<Vec<String>> {
        Some(names.iter().map(|name| (*name).to_owned()).collect())
    }

    fn badges_and_lines(trace: &RetrievalTrace) -> Vec<(Option<String>, String)> {
        taken(trace)
            .into_iter()
            .map(|row| (row.badge, row.line))
            .collect()
    }

    #[test]
    fn a_finished_search_names_what_each_step_produced() {
        let trace = RetrievalTrace {
            documents_searched: None,
            nearest: 8,
            seed_concepts: strings(&[
                "Black–Scholes model",
                "Volatility",
                "Itô's lemma",
                "Risk-free rate",
                "European option",
            ]),
            related_concepts: strings(&["Risk-neutral measure", "Hedging"]),
            candidates: Some(14),
            ranked: Some(14),
            kept: Some(8),
            passed_over: vec![(
                "Quanty Sample Notes, chapter 2: The Black–Scholes model".to_owned(),
                2,
            )],
            cited: strings(&["Table 1-1"]),
        };
        let some = |badge: &str, line: &str| (Some(badge.to_owned()), line.to_owned());
        assert_eq!(
            badges_and_lines(&trace),
            [
                some(
                    "8 items",
                    "The stored items closest in meaning to the question."
                ),
                some(
                    "5 concepts",
                    "Black–Scholes model, Volatility, Itô's lemma and 2 more"
                ),
                some("+6 items", "Risk-neutral measure, Hedging"),
                some(
                    "8 kept",
                    "14 ranked together. The cap passed over 2: Quanty Sample Notes, chapter 2: The Black–Scholes model."
                ),
                some("+1 item", "Table 1-1"),
            ]
        );
        assert!(
            taken(&trace)
                .iter()
                .all(|row| row.progress == Progress::Taken)
        );
    }

    fn badges(trace: &RetrievalTrace) -> Vec<Option<String>> {
        taken(trace).into_iter().map(|row| row.badge).collect()
    }

    #[test]
    fn a_missing_step_is_not_reached_and_a_real_zero_is_a_zero() {
        let empty_library = RetrievalTrace::default();
        let rows = taken(&empty_library);
        assert_eq!(rows[0].badge.as_deref(), Some("0 items"));
        assert_eq!(rows[0].line, "The library holds no items.");
        assert!(
            rows[1..]
                .iter()
                .all(|row| row.progress == Progress::NotReached
                    && row.badge.as_deref() == Some("Not reached"))
        );
        // A not reached row keeps its about line, so a person reads what the step would have done.
        assert_eq!(rows[1].line, ROWS[1].about);

        let no_document = RetrievalTrace {
            documents_searched: Some(0),
            ..RetrievalTrace::default()
        };
        assert_eq!(badges(&no_document)[0].as_deref(), Some("0 documents"));

        let no_item_matches = RetrievalTrace {
            documents_searched: Some(2),
            ..RetrievalTrace::default()
        };
        assert_eq!(
            taken(&no_item_matches)[0].line,
            "No item matches the filters."
        );

        let nothing_found = RetrievalTrace {
            nearest: 8,
            seed_concepts: Some(Vec::new()),
            related_concepts: Some(Vec::new()),
            candidates: Some(8),
            ranked: Some(8),
            kept: Some(8),
            cited: Some(Vec::new()),
            ..RetrievalTrace::default()
        };
        assert_eq!(
            badges(&nothing_found),
            ["8 items", "0 concepts", "+0 items", "8 kept", "+0 items"]
                .map(|text| Some(text.to_owned()))
        );
    }

    #[test]
    fn a_count_that_is_missing_has_no_badge_and_a_count_that_does_not_add_up_is_zero() {
        let half = RetrievalTrace {
            nearest: 9,
            related_concepts: Some(vec!["Hedging".to_owned()]),
            candidates: Some(4),
            kept: Some(1),
            ..RetrievalTrace::default()
        };
        let rows = taken(&half);
        assert_eq!(rows[2].badge.as_deref(), Some("+0 items"));
        assert_eq!(rows[3].badge.as_deref(), Some("1 kept"));
        assert_eq!(rows[3].line, "The cap passed over none.");
        let only_related = RetrievalTrace {
            related_concepts: Some(vec!["Hedging".to_owned()]),
            ..RetrievalTrace::default()
        };
        assert_eq!(taken(&only_related)[2].badge, None);
    }

    #[test]
    fn a_filter_and_a_cap_show_in_the_ranked_line() {
        let filtered = RetrievalTrace {
            documents_searched: Some(2),
            nearest: 3,
            candidates: Some(14),
            ranked: Some(11),
            kept: Some(6),
            passed_over: vec![("Notes".to_owned(), 3), ("Hull".to_owned(), 2)],
            ..RetrievalTrace::default()
        };
        let rows = taken(&filtered);
        assert_eq!(
            rows[0].line,
            "The closest items in the 2 documents with these labels."
        );
        assert_eq!(
            rows[3].line,
            "11 of 14 ranked together; the others lack these labels. The cap passed over 5: 3 from Notes; 2 from Hull."
        );
    }
}
