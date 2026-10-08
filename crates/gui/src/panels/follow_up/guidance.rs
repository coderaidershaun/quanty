//! Every word of Follow up and the rule that picks what shows under the box.

use crate::contract::{Answer, AskMode, Loadable, NothingFound};
use crate::state::AskSession;

pub(super) const TITLE: &str = "Follow up";
pub(super) const CAPTION: &str = "Each question is a new search";
pub(super) const NOTHING_YET: &str = "Nothing to follow up yet";
pub(super) const NOTHING_YET_HINT: &str =
    "Ask a question above. Questions to ask next appear here.";
pub(super) const BOX_LABEL: &str = "Follow-up question";
pub(super) const BOX_HINT: &str = "Ask another question (same mode and filters)…";
pub(super) const BOX_HOVER: &str =
    "quanty does not remember earlier questions. Name the subject in full.";
pub(super) const SEND_LABEL: &str = "Ask follow-up";
pub(super) const WAITING_LABEL: &str = "Waiting for suggestions";

const SEARCH_STOPPED: &str = "The search was stopped, so there is nothing to follow up.";
const SEARCH_FAILED: &str = "The search failed, so there is nothing to follow up.";
const WAITING: &str = "Suggestions come with the answer.";
const ANSWER_FAILED: &str = "The answer was not written, so no questions were suggested.";
const ANSWER_STOPPED: &str = "The answer was stopped, so no questions were suggested.";
const NONE_OFFERED: &str = "No further question was suggested for this answer.";
const UNANSWERED_LEAD: &str = "The sources do not answer this. Questions they may answer:";
const UNANSWERED_TITLE: &str = "No answer in the sources";
const UNANSWERED_BODY: &str =
    "The results do not answer this question. Ask it another way, or add a chapter that covers it.";
const NO_LABELS_TITLE: &str = "No document has these labels";
const NO_SOURCES_TITLE: &str = "No sources found";
const RESULTS_ONLY: &str = "Results only: no answer is written, so no questions are suggested. Choose Answer in the Ask bar to get them.";

#[derive(Debug)]
pub(super) struct Guidance<'a> {
    pub(super) message: Option<Message<'a>>,
    pub(super) suggestions: &'a [String],
}

#[derive(Debug)]
pub(super) enum Message<'a> {
    Waiting(&'static str),
    Line {
        text: &'static str,
        hint: Option<&'a str>,
    },
    Notice {
        title: &'static str,
        body: &'static str,
    },
}

impl Message<'_> {
    fn line(text: &'static str) -> Self {
        Message::Line { text, hint: None }
    }
}

/// `None` before the first ask: there is nothing to follow up, and no box.
pub(super) fn guidance(ask: &AskSession) -> Option<Guidance<'_>> {
    if ask.question.is_empty() {
        return None;
    }
    let message = match &ask.search {
        Loadable::Idle => Message::line(SEARCH_STOPPED),
        Loadable::Failed(failure) => Message::Line {
            text: SEARCH_FAILED,
            hint: Some(&failure.hint),
        },
        Loadable::Ready(reply) if reply.results.is_empty() => {
            no_sources(reply.trace.why_nothing_was_found())
        }
        Loadable::Loading | Loadable::Ready(_) if ask.mode == AskMode::ResultsOnly => {
            Message::line(RESULTS_ONLY)
        }
        Loadable::Loading => Message::Waiting(WAITING),
        Loadable::Ready(_) => return Some(of_answer(&ask.answer)),
    };
    Some(Guidance {
        message: Some(message),
        suggestions: &[],
    })
}

fn no_sources(reason: NothingFound) -> Message<'static> {
    let title = match reason {
        NothingFound::NoDocumentHasTheLabels => NO_LABELS_TITLE,
        NothingFound::NoItemMatchesTheFilters | NothingFound::LibraryHoldsNoItems => {
            NO_SOURCES_TITLE
        }
    };
    Message::Notice {
        title,
        body: reason.hint(),
    }
}

fn of_answer(answer: &Loadable<Answer>) -> Guidance<'_> {
    let (message, suggestions) = match answer {
        Loadable::Loading => (Some(Message::Waiting(WAITING)), &[][..]),
        Loadable::Failed(failure) => (
            Some(Message::Line {
                text: ANSWER_FAILED,
                hint: Some(&failure.hint),
            }),
            &[][..],
        ),
        Loadable::Idle => (Some(Message::line(ANSWER_STOPPED)), &[][..]),
        Loadable::Ready(answer) => match (answer.blocks.is_empty(), answer.follow_ups.is_empty()) {
            (true, true) => (
                Some(Message::Notice {
                    title: UNANSWERED_TITLE,
                    body: UNANSWERED_BODY,
                }),
                &[][..],
            ),
            (true, false) => (Some(Message::line(UNANSWERED_LEAD)), &answer.follow_ups[..]),
            (false, true) => (Some(Message::line(NONE_OFFERED)), &[][..]),
            (false, false) => (None, &answer.follow_ups[..]),
        },
    };
    Guidance {
        message,
        suggestions,
    }
}
