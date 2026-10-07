//! The question being asked, the three results that come back one after the other, and the
//! rules that keep an old reply from landing under a new question.

use std::collections::BTreeSet;

use super::HealthLevel;
use super::shared::{Shared, push_cancel};
use crate::contract::{
    Answer, AnswerBlock, AskDraft, AskMode, Command, ConceptGraph, ConceptId, Effect, Failure,
    Filters, ItemKind, Loadable, RequestId, ResultItem, SearchReply, Tab,
};

/// The ask that the three slots belong to. A new ask replaces the whole session.
#[derive(Debug, Clone, PartialEq, PartialOrd, Default)]
pub struct AskSession {
    /// Goes up by one on every new ask. Panels reset their own state when it changes.
    pub generation: u64,
    pub request: Option<RequestId>,
    /// As submitted.
    pub question: String,
    pub mode: AskMode,
    pub filters: Filters,
    pub search: Loadable<SearchReply>,
    pub graph: Loadable<ConceptGraph>,
    /// Stays `Idle` in mode `ResultsOnly`.
    pub answer: Loadable<Answer>,
    /// The number of the selected result.
    pub selected_result: Option<usize>,
    pub focused_concept: Option<ConceptId>,
}

impl AskSession {
    /// True while any slot is loading.
    pub fn is_running(&self) -> bool {
        self.search.is_loading() || self.graph.is_loading() || self.answer.is_loading()
    }

    /// Empty until the search is ready.
    pub fn results(&self) -> &[ResultItem] {
        match &self.search {
            Loadable::Ready(reply) => &reply.results,
            _ => &[],
        }
    }

    /// The result with this citation number.
    pub fn result(&self, number: usize) -> Option<&ResultItem> {
        self.results().iter().find(|result| result.number == number)
    }

    /// The answer as Markdown, for Share. `None` while there is no answer or it has no block.
    pub fn answer_markdown(&self) -> Option<String> {
        let answer = self.answer.ready()?;
        if answer.blocks.is_empty() {
            return None;
        }
        let title = answer.title.as_deref().unwrap_or(&self.question);
        let mut parts = vec![format!("# {title}")];
        let mut cited = BTreeSet::new();
        for block in &answer.blocks {
            match block {
                AnswerBlock::Heading(heading) => parts.push(format!("## {heading}")),
                AnswerBlock::Paragraph { text, cites } => {
                    let marks: String = cites.iter().map(|number| format!(" [{number}]")).collect();
                    parts.push(format!("{text}{marks}"));
                    cited.extend(cites);
                }
                AnswerBlock::Item(number) => {
                    if let Some(result) = self.result(*number) {
                        parts.push(card_markdown(result));
                        cited.insert(number);
                    }
                }
            }
        }
        let sources: Vec<String> = cited
            .into_iter()
            .filter_map(|number| self.result(*number))
            .map(source_line)
            .collect();
        let mut markdown = parts.join("\n\n");
        if !sources.is_empty() {
            markdown.push_str("\n\nSources\n");
            markdown.push_str(&sources.join("\n"));
        }
        Some(markdown)
    }
}

fn card_markdown(result: &ResultItem) -> String {
    let number = result.number;
    let name = match result.kind {
        ItemKind::Formula => return format!("$$ {} $$ [{number}]", result.text),
        ItemKind::Figure => "Figure",
        ItemKind::Table => "Table",
        ItemKind::Chunk => "Text",
    };
    format!("*{}* [{number}]", result.label.as_deref().unwrap_or(name))
}

fn source_line(result: &ResultItem) -> String {
    let page = result
        .printed_page
        .clone()
        .unwrap_or_else(|| format!("position {}", result.page));
    let mut line = format!("[{}] {}, p. {page}", result.number, result.doc_title);
    if let Some(label) = &result.label {
        line.push_str(&format!(" \u{2014} {label}"));
    }
    line
}

/// A slot that is still waiting goes back to idle.
fn stop_waiting<T>(slot: &mut Loadable<T>) {
    if slot.is_loading() {
        *slot = Loadable::Idle;
    }
}

impl Shared {
    pub(super) fn start_ask(&mut self, draft: AskDraft, effects: &mut Vec<Effect>) {
        if draft.question.trim().is_empty() {
            return;
        }
        let request = self.issue_request();
        if let (true, Some(old)) = (self.ask.is_running(), self.ask.request) {
            push_cancel(effects, old);
        }
        self.ask = AskSession {
            generation: self.ask.generation + 1,
            request: Some(request),
            question: draft.question.clone(),
            mode: draft.mode,
            filters: draft.filters.clone(),
            search: Loadable::Loading,
            graph: Loadable::Loading,
            answer: match draft.mode {
                AskMode::Answer => Loadable::Loading,
                AskMode::ResultsOnly => Loadable::Idle,
            },
            selected_result: None,
            focused_concept: None,
        };
        self.tab = Tab::Ask;
        effects.push(Effect::Send(Command::Ask {
            request,
            ask: draft,
        }));
    }

    pub(super) fn cancel_ask(&mut self, effects: &mut Vec<Effect>) {
        if !self.ask.is_running() {
            return;
        }
        stop_waiting(&mut self.ask.search);
        stop_waiting(&mut self.ask.graph);
        stop_waiting(&mut self.ask.answer);
        if let Some(request) = self.ask.request {
            push_cancel(effects, request);
        }
    }

    pub(super) fn select_result(&mut self, number: usize, effects: &mut Vec<Effect>) {
        let Some(result) = self.ask.result(number) else {
            return;
        };
        let (doc, page, piece) = (result.doc, result.page, result.piece);
        self.ask.selected_result = Some(number);
        self.open_source(doc, page, piece, effects);
    }

    pub(super) fn step_result(&mut self, delta: i32, effects: &mut Vec<Effect>) {
        let results = self.ask.results();
        if results.is_empty() {
            return;
        }
        let current = self
            .ask
            .selected_result
            .and_then(|number| results.iter().position(|result| result.number == number));
        let step = isize::try_from(delta).unwrap_or_default();
        let index = current.map_or(0, |at| {
            at.saturating_add_signed(step).min(results.len() - 1)
        });
        if let Some(number) = results.get(index).map(|result| result.number) {
            self.select_result(number, effects);
        }
    }

    pub(super) fn search_arrived(
        &mut self,
        request: RequestId,
        result: Result<SearchReply, Failure>,
        effects: &mut Vec<Effect>,
    ) {
        if self.ask.request != Some(request) || !self.ask.search.is_loading() {
            return;
        }
        match result {
            Ok(reply) => {
                let found_nothing = reply.results.is_empty();
                let was_down = self.health.level() == HealthLevel::Down;
                self.ask.search = Loadable::Ready(reply);
                if found_nothing {
                    self.ask.graph = Loadable::Ready(ConceptGraph::default());
                    self.ask.answer = Loadable::Idle;
                }
                if was_down {
                    self.recheck_health(effects);
                }
            }
            Err(failure) => {
                self.mark_down(&failure);
                self.ask.search = Loadable::Failed(failure);
                self.ask.graph = Loadable::Idle;
                self.ask.answer = Loadable::Idle;
            }
        }
    }

    pub(super) fn graph_arrived(
        &mut self,
        request: RequestId,
        result: Result<ConceptGraph, Failure>,
    ) {
        if self.ask.request != Some(request) || !self.ask.graph.is_loading() {
            return;
        }
        if let Err(failure) = &result {
            self.mark_down(failure);
        }
        self.ask.graph = result.into();
    }

    pub(super) fn answer_arrived(&mut self, request: RequestId, result: Result<Answer, Failure>) {
        if self.ask.request != Some(request) || !self.ask.answer.is_loading() {
            return;
        }
        if let Err(failure) = &result {
            self.mark_down(failure);
        }
        self.ask.answer = result.into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{Command, Event, Intent};
    use crate::testkit::sample;
    use uuid::Uuid;

    fn draft(question: &str) -> AskDraft {
        AskDraft {
            question: question.to_owned(),
            ..AskDraft::default()
        }
    }

    /// A result on page `number + 2`, in a document called "Notes".
    fn item(
        number: usize,
        kind: ItemKind,
        label: Option<&str>,
        printed: Option<&str>,
    ) -> ResultItem {
        ResultItem {
            number,
            kind,
            page: u32::try_from(number + 2).unwrap_or(1),
            doc_title: "Notes".to_owned(),
            label: label.map(str::to_owned),
            printed_page: printed.map(str::to_owned),
            text: r"\int_0^1 x\,dx".to_owned(),
            ..sample::search_reply().results.remove(0)
        }
    }

    fn found(request: RequestId, count: usize) -> Event {
        let results = (1..=count)
            .map(|number| item(number, ItemKind::Chunk, None, None))
            .collect();
        Event::Search {
            request,
            result: Ok(SearchReply {
                results,
                ..SearchReply::default()
            }),
        }
    }

    fn run(shared: &mut Shared, intent: Intent) -> Vec<Effect> {
        let mut effects = Vec::new();
        shared.apply_intent(intent, &mut effects);
        effects
    }

    fn deliver(shared: &mut Shared, event: Event) {
        shared.apply_event(event, &mut Vec::new());
    }

    /// Asks `question` and gives the id the backend was told to work on.
    fn ask(shared: &mut Shared, question: &str) -> RequestId {
        match run(shared, Intent::Ask(draft(question))).last() {
            Some(Effect::Send(Command::Ask { request, .. })) => *request,
            other => panic!("expected an ask, got {other:?}"),
        }
    }

    #[test]
    fn an_ask_cancels_the_one_before_and_shows_results_before_the_answer() {
        let mut shared = Shared::default();
        assert!(run(&mut shared, Intent::Ask(draft("   "))).is_empty());
        assert_eq!(shared.ask.generation, 0);

        let first = ask(&mut shared, "first");
        assert_eq!(
            (shared.ask.generation, shared.ask.request),
            (1, Some(first))
        );
        assert!(shared.ask.search.is_loading() && shared.ask.graph.is_loading());
        assert!(shared.ask.answer.is_loading());

        let effects = run(&mut shared, Intent::Ask(draft("second")));
        assert_eq!(effects[0], Effect::Send(Command::Cancel(first)));
        let second = shared.ask.request.expect("the second ask is running");
        assert_eq!(
            (shared.ask.generation, shared.ask.question.as_str()),
            (2, "second")
        );

        // A reply of the old ask is dropped.
        deliver(&mut shared, found(first, 2));
        assert!(shared.ask.search.is_loading());

        // The results show while the answer is still loading.
        deliver(&mut shared, found(second, 3));
        assert_eq!(shared.ask.results().len(), 3);
        assert!(shared.ask.answer.is_loading());
        assert_eq!(shared.ask.result(2).map(|found| found.page), Some(4));
        let graph = Ok(sample::concept_graph());
        deliver(
            &mut shared,
            Event::Graph {
                request: second,
                result: graph,
            },
        );
        let answer = Ok(Answer::default());
        deliver(
            &mut shared,
            Event::Answer {
                request: second,
                result: answer,
            },
        );
        assert!(!shared.ask.is_running());
        assert!(shared.ask.graph.ready().is_some() && shared.ask.answer.ready().is_some());

        // A search that finds nothing needs no graph and no answer.
        let empty = ask(&mut shared, "nothing");
        deliver(&mut shared, found(empty, 0));
        assert!(shared.ask.search.ready().is_some());
        assert!(
            shared
                .ask
                .graph
                .ready()
                .is_some_and(|graph| graph.nodes.is_empty())
        );
        assert_eq!(shared.ask.answer, Loadable::Idle);

        // A failed search leaves the other two slots idle.
        let broken = ask(&mut shared, "broken");
        let failure = Failure::internal("no route");
        let result = Err(failure.clone());
        deliver(
            &mut shared,
            Event::Search {
                request: broken,
                result,
            },
        );
        assert_eq!(shared.ask.search, Loadable::Failed(failure));
        assert_eq!(
            (&shared.ask.graph, &shared.ask.answer),
            (&Loadable::Idle, &Loadable::Idle)
        );

        // Results only: the answer never loads.
        let only = AskDraft {
            mode: AskMode::ResultsOnly,
            ..draft("results")
        };
        run(&mut shared, Intent::Ask(only));
        assert_eq!(shared.ask.answer, Loadable::Idle);
        assert!(shared.ask.is_running());

        // Cancel empties every waiting slot and tells the backend once.
        let running = shared.ask.request.expect("an ask is running");
        let effects = run(&mut shared, Intent::CancelAsk);
        assert_eq!(effects, vec![Effect::Send(Command::Cancel(running))]);
        assert!(!shared.ask.is_running());
        deliver(&mut shared, found(running, 1));
        assert_eq!(shared.ask.search, Loadable::Idle);
        assert!(run(&mut shared, Intent::CancelAsk).is_empty());

        // Selecting a result opens its page; stepping walks the list and stops at its ends.
        let pick = ask(&mut shared, "pick");
        deliver(&mut shared, found(pick, 3));
        assert!(run(&mut shared, Intent::SelectResult(9)).is_empty());
        assert_eq!(
            (shared.ask.selected_result, shared.cues.source_shows),
            (None, 0)
        );
        let effects = run(&mut shared, Intent::SelectResult(2));
        assert_eq!(shared.ask.selected_result, Some(2));
        assert!(matches!(
            effects.as_slice(),
            [Effect::Send(Command::LoadPage { page: 4, .. })]
        ));
        for (step, selected) in [(1, 3), (1, 3), (-5, 1)] {
            run(&mut shared, Intent::StepResult(step));
            assert_eq!(shared.ask.selected_result, Some(selected));
        }
        assert_eq!(
            shared.cues.source_shows, 4,
            "a select and each step show the source"
        );
        // A new ask forgets the selection.
        ask(&mut shared, "again");
        assert_eq!(shared.ask.selected_result, None);
        let concept = ConceptId(Uuid::from_u128(7));
        run(&mut shared, Intent::FocusConcept(Some(concept)));
        assert_eq!(shared.ask.focused_concept, Some(concept));
    }

    fn shared_with(results: Vec<ResultItem>, answer: Option<Answer>) -> Shared {
        let mut shared = Shared::default();
        shared.ask.question = "What is it?".to_owned();
        shared.ask.search = Loadable::Ready(SearchReply {
            results,
            ..SearchReply::default()
        });
        shared.ask.answer = answer.map_or(Loadable::Idle, Loadable::Ready);
        shared
    }

    #[test]
    fn a_shared_answer_carries_its_marks_and_its_source_list() {
        let results = vec![
            item(1, ItemKind::Formula, Some("Formula (2.4)"), Some("41")),
            item(2, ItemKind::Figure, Some("Figure 3.1"), None),
            item(3, ItemKind::Table, None, None),
            item(4, ItemKind::Chunk, None, Some("7")),
        ];
        let paragraph = |text: &str, cites: Vec<usize>| AnswerBlock::Paragraph {
            text: text.to_owned(),
            cites,
        };
        let answer = Answer {
            title: Some("Black\u{2013}Scholes".to_owned()),
            blocks: vec![
                AnswerBlock::Heading("Assumptions".to_owned()),
                paragraph("Prices move randomly.", vec![4, 1]),
                AnswerBlock::Item(1),
                AnswerBlock::Item(2),
                AnswerBlock::Item(3),
                AnswerBlock::Item(9),
                paragraph("No marks here.", Vec::new()),
            ],
            follow_ups: Vec::new(),
        };
        let mut shared = shared_with(results, Some(answer.clone()));

        let expected = "# Black\u{2013}Scholes\n\n## Assumptions\n\nPrices move randomly. [4] [1]\n\n\
            $$ \\int_0^1 x\\,dx $$ [1]\n\n*Figure 3.1* [2]\n\n*Table* [3]\n\nNo marks here.\n\n\
            Sources\n[1] Notes, p. 41 \u{2014} Formula (2.4)\n[2] Notes, p. position 4 \u{2014} Figure 3.1\n\
            [3] Notes, p. position 5\n[4] Notes, p. 7";
        assert_eq!(shared.ask.answer_markdown().as_deref(), Some(expected));
        let effects = run(&mut shared, Intent::ShareAnswer);
        assert_eq!(effects, vec![Effect::CopyText(expected.to_owned())]);

        // With no title, the question stands in its place.
        let untitled = Answer {
            title: None,
            ..answer
        };
        let shared = shared_with(vec![item(1, ItemKind::Chunk, None, None)], Some(untitled));
        let markdown = shared.ask.answer_markdown().expect("an answer to share");
        assert!(markdown.starts_with("# What is it?\n\n"));

        // No answer, or one with no block, shares nothing.
        let mut empty = shared_with(Vec::new(), Some(Answer::default()));
        assert_eq!(empty.ask.answer_markdown(), None);
        assert!(run(&mut empty, Intent::ShareAnswer).is_empty());
        assert_eq!(shared_with(Vec::new(), None).ask.answer_markdown(), None);
    }
}
