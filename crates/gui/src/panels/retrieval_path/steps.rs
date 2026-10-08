//! Every word of the path and the rule that picks it: what the panel says when it has no steps
//! to show, and the five steps themselves.

use crate::contract::{Failure, Loadable, RetrievalTrace};
use crate::state::AskSession;
use crate::theme::{Icon, Kind, Tone};

pub(super) const TITLE: &str = "Retrieval Path";
pub(super) const SEARCHING: &str = "Searching";
pub(super) const FAILED: &str = "The search failed";

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

fn added(count: usize) -> String {
    format!("+{}", counted(count, "item", "items"))
}

fn counted(count: usize, one: &str, many: &str) -> String {
    let noun = if count == 1 { one } else { many };
    format!("{count} {noun}")
}

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

/// A line of `None` leaves the row's own about line.
struct Outcome {
    progress: Progress,
    badge: Option<String>,
    line: Option<String>,
}

impl Outcome {
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
