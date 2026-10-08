//! Names what the pane has to show from the ask alone, so drawing never guesses.

use crate::contract::{Answer, AskMode, Failure, ItemKind, Loadable};
use crate::state::AskSession;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum AnswerTab {
    Answer,
    Results,
    Formulas,
    Figures,
    Tables,
}

impl AnswerTab {
    pub(super) const ALL: [AnswerTab; 5] = [
        AnswerTab::Answer,
        AnswerTab::Results,
        AnswerTab::Formulas,
        AnswerTab::Figures,
        AnswerTab::Tables,
    ];

    pub(super) const fn opened_by(mode: AskMode) -> AnswerTab {
        match mode {
            AskMode::Answer => AnswerTab::Answer,
            AskMode::ResultsOnly => AnswerTab::Results,
        }
    }

    pub(super) const fn index(self) -> usize {
        self as usize
    }

    pub(super) const fn label(self) -> &'static str {
        match self {
            AnswerTab::Answer => "Answer",
            AnswerTab::Results => "Results",
            AnswerTab::Formulas => "Formulas",
            AnswerTab::Figures => "Figures",
            AnswerTab::Tables => "Tables",
        }
    }

    /// The Answer tab lists results of every kind while the answer is not written.
    pub(super) const fn lists(self, kind: ItemKind) -> bool {
        match self {
            AnswerTab::Answer | AnswerTab::Results => true,
            AnswerTab::Formulas => matches!(kind, ItemKind::Formula),
            AnswerTab::Figures => matches!(kind, ItemKind::Figure),
            AnswerTab::Tables => matches!(kind, ItemKind::Table),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum Phase<'a> {
    Idle,
    Searching,
    SearchStopped,
    SearchFailed(&'a Failure),
    NoResults,
    Found { written: Written<'a> },
}

/// What became of the answer once the results stand.
#[derive(Debug, Clone, Copy)]
pub(super) enum Written<'a> {
    NotAsked,
    Writing,
    Ready(&'a Answer),
    Empty,
    Failed(&'a Failure),
    Stopped,
}

impl<'a> Phase<'a> {
    pub(super) fn of(ask: &'a AskSession) -> Phase<'a> {
        let reply = match &ask.search {
            Loadable::Idle if ask.generation == 0 => return Phase::Idle,
            Loadable::Idle => return Phase::SearchStopped,
            Loadable::Loading => return Phase::Searching,
            Loadable::Failed(failure) => return Phase::SearchFailed(failure),
            Loadable::Ready(reply) => reply,
        };
        if reply.results.is_empty() {
            return Phase::NoResults;
        }
        let written = match &ask.answer {
            Loadable::Loading => Written::Writing,
            Loadable::Failed(failure) => Written::Failed(failure),
            Loadable::Ready(answer) if answer.blocks.is_empty() => Written::Empty,
            Loadable::Ready(answer) => Written::Ready(answer),
            Loadable::Idle => match ask.mode {
                AskMode::ResultsOnly => Written::NotAsked,
                AskMode::Answer => Written::Stopped,
            },
        };
        Phase::Found { written }
    }
}
