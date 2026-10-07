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

    /// The tab that is open until the person chooses another.
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

    /// True when this tab lists results of this kind. The Answer tab lists them all while the
    /// answer is not written.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{AnswerBlock, SearchReply};
    use crate::testkit::sample;

    fn failure() -> Failure {
        sample::failure(crate::contract::FailureKind::Internal)
    }

    fn ask(search: Loadable<SearchReply>, answer: Loadable<Answer>) -> AskSession {
        AskSession {
            generation: 1,
            search,
            answer,
            ..AskSession::default()
        }
    }

    fn found() -> Loadable<SearchReply> {
        Loadable::Ready(sample::search_reply())
    }

    fn written(session: &AskSession) -> Option<&'static str> {
        let Phase::Found { written, .. } = Phase::of(session) else {
            return None;
        };
        Some(match written {
            Written::NotAsked => "not asked",
            Written::Writing => "writing",
            Written::Ready(_) => "ready",
            Written::Empty => "empty",
            Written::Failed(_) => "failed",
            Written::Stopped => "stopped",
        })
    }

    #[test]
    fn the_search_alone_names_the_states_with_no_result() {
        let never = AskSession::default();
        assert!(matches!(Phase::of(&never), Phase::Idle));
        let cancelled = ask(Loadable::Idle, Loadable::Idle);
        assert!(matches!(Phase::of(&cancelled), Phase::SearchStopped));
        let loading = ask(Loadable::Loading, Loadable::Loading);
        assert!(matches!(Phase::of(&loading), Phase::Searching));
        let broken = ask(Loadable::Failed(failure()), Loadable::Idle);
        assert!(matches!(Phase::of(&broken), Phase::SearchFailed(_)));
        let nothing = ask(Loadable::Ready(SearchReply::default()), Loadable::Idle);
        assert!(matches!(Phase::of(&nothing), Phase::NoResults));
    }

    #[test]
    fn the_answer_names_what_stands_under_the_results() {
        let block = Answer {
            blocks: vec![AnswerBlock::Item(1)],
            ..Answer::default()
        };
        let cases = [
            (Loadable::Loading, "writing"),
            (Loadable::Failed(failure()), "failed"),
            (Loadable::Ready(block), "ready"),
            (Loadable::Ready(Answer::default()), "empty"),
            (Loadable::Idle, "stopped"),
        ];
        for (answer, expected) in cases {
            assert_eq!(written(&ask(found(), answer)), Some(expected));
        }
        let results_only = AskSession {
            mode: AskMode::ResultsOnly,
            ..ask(found(), Loadable::Idle)
        };
        assert_eq!(written(&results_only), Some("not asked"));
    }

    #[test]
    fn a_tab_lists_its_own_kind_and_the_mode_opens_a_tab() {
        assert_eq!(AnswerTab::opened_by(AskMode::Answer), AnswerTab::Answer);
        assert_eq!(
            AnswerTab::opened_by(AskMode::ResultsOnly),
            AnswerTab::Results
        );
        let kinds = [
            ItemKind::Chunk,
            ItemKind::Formula,
            ItemKind::Figure,
            ItemKind::Table,
        ];
        let listed = |tab: AnswerTab| kinds.map(|kind| tab.lists(kind));
        assert_eq!(listed(AnswerTab::Results), [true; 4]);
        assert_eq!(listed(AnswerTab::Formulas), [false, true, false, false]);
        assert_eq!(listed(AnswerTab::Figures), [false, false, true, false]);
        assert_eq!(listed(AnswerTab::Tables), [false, false, false, true]);
        for (at, tab) in AnswerTab::ALL.into_iter().enumerate() {
            assert_eq!(tab.index(), at);
        }
    }
}
