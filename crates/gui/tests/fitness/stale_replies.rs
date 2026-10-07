//! Checks that what the window shows always belongs to the question that was asked last.
//!
//! This is a fitness test because the failure makes no noise: an answer to an old question
//! would be shown under a new one, as if it were right.

use gui::contract::{
    Answer, AskDraft, AskMode, Command, ConceptGraph, Effect, Event, Failure, Intent, Loadable,
    RequestId,
};
use gui::state::Shared;
use gui::testkit::sample;
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Step {
    Ask(AskMode),
    Cancel,
    Search(usize, bool),
    Graph(usize),
    Answer(usize, bool),
}

fn step() -> impl Strategy<Value = Step> {
    let mode = prop_oneof![Just(AskMode::Answer), Just(AskMode::ResultsOnly)];
    prop_oneof![
        mode.prop_map(Step::Ask),
        Just(Step::Cancel),
        (any::<usize>(), any::<bool>()).prop_map(|(index, ok)| Step::Search(index, ok)),
        any::<usize>().prop_map(Step::Graph),
        (any::<usize>(), any::<bool>()).prop_map(|(index, ok)| Step::Answer(index, ok)),
    ]
}

/// A reply that carries the question it answers in every place that is shown.
fn search_reply_for(question: &str) -> gui::contract::SearchReply {
    let mut reply = sample::search_reply();
    for result in &mut reply.results {
        result.text = question.to_owned();
    }
    reply
}

fn graph_for(question: &str) -> ConceptGraph {
    let mut graph = sample::concept_graph();
    for node in &mut graph.nodes {
        node.label = question.to_owned();
    }
    graph
}

fn answer_for(question: &str) -> Answer {
    Answer {
        title: Some(question.to_owned()),
        ..sample::answer()
    }
}

/// Any request issued so far, chosen by `index`. `None` before the first ask.
fn pick(issued: &[(RequestId, String)], index: usize) -> Option<&(RequestId, String)> {
    issued.get(index.checked_rem(issued.len())?)
}

fn check(shared: &Shared) {
    let question = &shared.ask.question;
    if let Loadable::Ready(reply) = &shared.ask.search {
        assert!(reply.results.iter().all(|result| &result.text == question));
    }
    if let Loadable::Ready(graph) = &shared.ask.graph {
        assert!(graph.nodes.iter().all(|node| &node.label == question));
    }
    if let Loadable::Ready(answer) = &shared.ask.answer {
        assert_eq!(answer.title.as_ref(), Some(question));
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    /// However asks, cancels and replies come in, whatever is ready belongs to the current ask,
    /// and a cancel leaves nothing loading.
    #[test]
    fn what_is_shown_always_belongs_to_the_current_ask(
        steps in prop::collection::vec(step(), 1..40),
    ) {
        let mut shared = Shared::default();
        let mut issued: Vec<(RequestId, String)> = Vec::new();
        for step in steps {
            let mut effects = Vec::new();
            match step {
                Step::Ask(mode) => {
                    let question = format!("question {}", issued.len() + 1);
                    let draft = AskDraft { question, mode, ..AskDraft::default() };
                    shared.apply_intent(Intent::Ask(draft), &mut effects);
                    for effect in &effects {
                        if let Effect::Send(Command::Ask { request, ask }) = effect {
                            issued.push((*request, ask.question.clone()));
                        }
                    }
                }
                Step::Cancel => {
                    shared.apply_intent(Intent::CancelAsk, &mut effects);
                    prop_assert!(!shared.ask.is_running());
                }
                Step::Search(index, ok) => {
                    if let Some((request, question)) = pick(&issued, index) {
                        let result = if ok {
                            Ok(search_reply_for(question))
                        } else {
                            Err(Failure::internal("search failed"))
                        };
                        shared.apply_event(Event::Search { request: *request, result }, &mut effects);
                    }
                }
                Step::Graph(index) => {
                    if let Some((request, question)) = pick(&issued, index) {
                        let result = Ok(graph_for(question));
                        shared.apply_event(Event::Graph { request: *request, result }, &mut effects);
                    }
                }
                Step::Answer(index, ok) => {
                    if let Some((request, question)) = pick(&issued, index) {
                        let result = if ok {
                            Ok(answer_for(question))
                        } else {
                            Err(Failure::internal("answer failed"))
                        };
                        shared.apply_event(Event::Answer { request: *request, result }, &mut effects);
                    }
                }
            }
            check(&shared);
        }
    }
}
