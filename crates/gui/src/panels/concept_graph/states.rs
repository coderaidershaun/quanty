//! What the panel says when there is no picture: nothing asked yet, a wait, a failure, or a
//! graph with nothing to relate.

use eframe::egui;

use crate::contract::Loadable;
use crate::panels::PanelCx;
use crate::theme::Icon;
use crate::widgets::Placeholder;

pub(super) fn show(ui: &mut egui::Ui, cx: &PanelCx<'_>) {
    let ask = &cx.shared.ask;
    match &ask.graph {
        Loadable::Idle if ask.question.is_empty() => {
            Placeholder::empty(Icon::GRAPH, "No graph yet")
                .hint("Ask a question to see how its concepts relate.")
                .show(ui);
        }
        Loadable::Idle => {
            let hint = if ask.search.failure().is_some() {
                "The search failed, so there is nothing to relate."
            } else {
                "The search was stopped."
            };
            Placeholder::empty(Icon::GRAPH, "No graph for this question")
                .hint(hint)
                .show(ui);
        }
        Loadable::Loading => {
            Placeholder::loading("Reading the concept graph").show(ui);
        }
        Loadable::Failed(failure) => {
            Placeholder::error("The concept graph could not be read")
                .hint(&failure.hint)
                .show(ui)
                .response
                .on_hover_text(&failure.detail);
        }
        Loadable::Ready(_) => ready_without_concepts(ui, cx),
    }
}

/// A graph that came back with no concept in it.
fn ready_without_concepts(ui: &mut egui::Ui, cx: &PanelCx<'_>) {
    if cx.shared.ask.results().is_empty() {
        Placeholder::empty(Icon::GRAPH, "Nothing to relate")
            .hint("The search found no sources.")
            .show(ui);
    } else {
        Placeholder::empty(Icon::GRAPH, "No concepts for these results")
            .hint("The items that were found have no stored concepts.")
            .show(ui);
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::accesskit::Role;
    use egui_kittest::Harness;
    use egui_kittest::kittest::{NodeT as _, Queryable};

    use super::super::samples::{self, DEFAULT, answered, drawn};
    use crate::contract::{ConceptGraph, Event, FailureKind, Intent, NodeKind, SearchReply};
    use crate::state::Shared;
    use crate::testkit::{self, Host, sample};

    /// The panel with `shared`, drawn and at rest, which means a spinner may still be asking.
    fn shown(shared: Shared) -> Harness<'static, Host> {
        let (mut harness, _) = drawn(DEFAULT, shared);
        harness.run_ok();
        harness
    }

    /// True when the panel has a node whose name has `text` in it.
    fn names(harness: &Harness<'_, Host>, text: &str) -> bool {
        harness.query_all_by_label_contains(text).next().is_some()
    }

    /// The panel says `title` and `hint`, has its title row, and has no picture to reset.
    fn says(harness: &Harness<'_, Host>, title: &str, hint: &str) {
        assert!(names(harness, title), "no `{title}`");
        assert!(names(harness, hint), "no `{hint}`");
        assert!(names(harness, "Concept Graph"));
        let reset = harness.get_by_label("Reset view");
        assert!(reset.accesskit_node().is_disabled());
        assert!(!names(harness, "Related"), "a legend with no picture");
    }

    #[test]
    fn every_state_says_what_it_is() {
        let shared = Shared::default();
        let mut harness = shown(shared);
        says(
            &harness,
            "No graph yet",
            "Ask a question to see how its concepts relate.",
        );
        testkit::save_png(&mut harness, "concept-graph-idle");

        let mut shared = testkit::asked("What is vega?");
        let request = shared.ask.request.expect("an ask is running");
        let failure = sample::failure(FailureKind::QdrantDown);
        let event = Event::Search {
            request,
            result: Err(failure),
        };
        shared.apply_event(event, &mut Vec::new());
        let harness = shown(shared);
        says(
            &harness,
            "No graph for this question",
            "The search failed, so there is nothing to relate.",
        );

        let mut shared = testkit::asked("What is vega?");
        shared.apply_intent(Intent::CancelAsk, &mut Vec::new());
        let harness = shown(shared);
        says(
            &harness,
            "No graph for this question",
            "The search was stopped.",
        );

        let mut harness = shown(testkit::asked("What is vega?"));
        let waits = harness
            .query_all_by_label_contains("Reading the concept graph")
            .any(|node| node.accesskit_node().role() == Role::ProgressIndicator);
        assert!(waits, "no spinner that says what is being read");
        testkit::save_png(&mut harness, "concept-graph-loading");

        let mut shared = testkit::searched(sample::search_reply());
        let request = shared.ask.request.expect("an ask is running");
        let failure = sample::failure(FailureKind::FalkorDbDown);
        let event = Event::Graph {
            request,
            result: Err(failure.clone()),
        };
        shared.apply_event(event, &mut Vec::new());
        let mut harness = shown(shared);
        says(
            &harness,
            "The concept graph could not be read",
            &failure.hint,
        );
        testkit::save_png(&mut harness, "concept-graph-failed");

        let results_with_no_concepts = ConceptGraph {
            nodes: vec![samples::item(1, NodeKind::Formula, "Formula (2.3)")],
            edges: Vec::new(),
        };
        let mut harness = shown(answered(results_with_no_concepts));
        says(
            &harness,
            "No concepts for these results",
            "The items that were found have no stored concepts.",
        );
        testkit::save_png(&mut harness, "concept-graph-empty");

        let mut harness = shown(testkit::searched(SearchReply::default()));
        says(
            &harness,
            "Nothing to relate",
            "The search found no sources.",
        );
        testkit::save_png(&mut harness, "concept-graph-no-sources");

        // One concept stands alone in the middle, with no link and one entry in the legend.
        let mut harness = shown(answered(samples::one_node()));
        harness.get_by_label("Concept Black–Scholes model");
        harness.get_by_label("Concept");
        assert!(!names(&harness, "Related") && !names(&harness, "Formula"));
        testkit::save_png(&mut harness, "concept-graph-one-node");
    }
}
