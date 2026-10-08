//! A service that is down, a key that is missing, a library that is empty and a page that is
//! gone are each said where they happen, with what to do, and the rest of the screen stays.

use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::Queryable as _;
use gui::app::layout::DEFAULT_WINDOW;
use gui::contract::{Command, FailureKind, Loadable};
use gui::testkit;

use super::recording::{self, Seen};
use super::{Window, click, failures, has, node, node_in, panels, says, shared};

/// The health check has no button in any scene.
fn open(scene: &str) -> (Window, Seen) {
    let (mut harness, seen) = recording::open(scene, DEFAULT_WINDOW);
    testkit::settle(&mut harness);
    assert!(
        harness.query_all_by_label("Check again").next().is_none(),
        "`{scene}` has a button named `Check again`"
    );
    (harness, seen)
}

fn page_loads(seen: &Seen) -> usize {
    seen.count(|command| matches!(command, Command::LoadPage { .. }))
}

fn says_hint_of<T>(harness: &Window, slot: &Loadable<T>) -> bool {
    let failure = slot.failure().expect("the slot has failed");
    says(harness, &failure.hint)
}

fn says_in(harness: &Window, words: &str, area: egui::Rect) -> bool {
    harness
        .query_all_by_label_contains(words)
        .any(|node| area.contains(node.rect().center()))
}

fn assert_nothing_is_waiting(harness: &Window) {
    let shared = shared(harness);
    assert!(
        !shared.ask.search.is_loading()
            && !shared.ask.graph.is_loading()
            && !shared.ask.answer.is_loading()
            && !shared.library.catalogue.is_loading()
            && !shared.source.page.is_loading()
            && !shared.source.concepts.is_loading(),
        "a slot is still loading: {shared:#?}"
    );
}

fn assert_no_failure(harness: &Window) {
    let held = failures(shared(harness));
    assert!(held.is_empty(), "nothing should have failed: {held:?}");
}

fn type_and_ask(harness: &mut Window, question: &str) {
    node(harness, Role::TextInput, "Question").click();
    node(harness, Role::TextInput, "Question").type_text(question);
    harness.run_ok();
    harness.key_press(egui::Key::Enter);
    testkit::settle(harness);
}

fn try_again_in(harness: &mut Window, area: egui::Rect) {
    node_in(harness, Role::Button, "Try again", area)
        .expect("the panel offers Try again")
        .click();
    testkit::settle(harness);
}

fn a_failed_search_says_why_and_asking_again_needs_no_other_step() {
    let (mut harness, _) = open("search-failed");
    let ask = &shared(&harness).ask;
    assert!(says_hint_of(&harness, &ask.search));
    assert_eq!(
        (&ask.graph, &ask.answer),
        (&Loadable::Idle, &Loadable::Idle)
    );
    assert_nothing_is_waiting(&harness);
    let before = ask.generation;
    click(&mut harness, Role::Button, "Ask");
    assert_eq!(shared(&harness).ask.generation, before + 1);
}

fn a_failed_answer_leaves_the_results() {
    let (harness, _) = open("answer-failed");
    let ask = &shared(&harness).ask;
    assert!(says_hint_of(&harness, &ask.answer));
    assert!(!ask.results().is_empty());
    assert!(has(&harness, Role::Button, "Result 1"));
}

fn stores_that_are_down_leave_the_page_that_is_on_disk() {
    let panels = panels(DEFAULT_WINDOW);
    let (mut harness, seen) = open("stores-down");
    let held = shared(&harness);
    // The two failures can carry the same hint, so each is looked for in its own panel.
    let stores = [
        (held.library.catalogue.failure(), panels.ask_bar),
        (held.ask.search.failure(), panels.answer),
    ];
    for (failure, panel) in stores {
        let failure = failure.expect("the store is down");
        assert!(matches!(
            failure.kind,
            FailureKind::QdrantDown | FailureKind::FalkorDbDown
        ));
        assert!(
            says_in(&harness, &failure.hint, panel),
            "the panel says what to do: {}",
            failure.hint
        );
    }
    assert!(node_in(&harness, Role::Button, "Try again", panels.ask_bar).is_some());

    let page = held.source.page.ready().expect("the page loads from disk");
    let picture = format!("Picture of page {}", page.page);
    assert_eq!(
        held.source.concepts.failure().map(|failure| failure.kind),
        Some(FailureKind::FalkorDbDown)
    );
    click(&mut harness, Role::Tab, "Concepts");
    assert!(says_hint_of(&harness, &shared(&harness).source.concepts));
    let before = page_loads(&seen);
    try_again_in(&mut harness, panels.source);
    assert_eq!(
        page_loads(&seen),
        before + 1,
        "Try again asks for the page again"
    );
    click(&mut harness, Role::Tab, "Page");
    assert!(has(&harness, Role::Image, &picture), "the page stays shown");
}

fn a_first_run_and_an_empty_library_are_not_failures() {
    let (mut harness, _) = open("first-run");
    let catalogue = shared(&harness).library.catalogue.ready();
    assert_eq!(catalogue.map(|found| found.documents().count()), Some(0));
    assert!(says(&harness, "library is empty"));
    assert_no_failure(&harness);
    type_and_ask(&mut harness, "What is delta?");
    let failure = shared(&harness)
        .ask
        .search
        .failure()
        .expect("no key, no search");
    assert!(says(&harness, &failure.hint.clone()));

    let (mut harness, _) = open("empty-library");
    let catalogue = shared(&harness).library.catalogue.ready();
    assert_eq!(catalogue.map(|found| found.documents().count()), Some(0));
    assert!(says(&harness, "library is empty"));
    assert_no_failure(&harness);
    type_and_ask(&mut harness, "What is delta?");
    let found = shared(&harness)
        .ask
        .search
        .ready()
        .expect("the search came back");
    assert!(found.results.is_empty());
    assert_no_failure(&harness);
}

fn a_search_that_finds_nothing_says_which_kind_of_nothing() {
    let (harness, _) = open("no-sources");
    let found = shared(&harness)
        .ask
        .search
        .ready()
        .expect("the search came back");
    assert!(found.results.is_empty());
    assert!(says(&harness, "No sources found"));

    let (harness, _) = open("no-labels");
    let found = shared(&harness)
        .ask
        .search
        .ready()
        .expect("the search came back");
    assert!(found.results.is_empty() && found.trace.documents_searched == Some(0));
    assert!(says(&harness, "No document has these labels"));
    assert!(!says(&harness, "No sources found"));
}

fn a_page_that_is_gone_names_what_to_do_and_leaves_the_answer() {
    let (mut harness, seen) = open("source-missing");
    click(&mut harness, Role::Button, "Citation 1");
    let failure = shared(&harness)
        .source
        .page
        .failure()
        .expect("the page is gone");
    assert_eq!(failure.kind, FailureKind::SourceMissing);
    assert!(failure.hint.contains("rag-ingest"));
    assert!(says(&harness, &failure.hint.clone()));
    let before = page_loads(&seen);
    try_again_in(&mut harness, panels(DEFAULT_WINDOW).source);
    assert_eq!(page_loads(&seen), before + 1);
    let ask = &shared(&harness).ask;
    assert!(ask.answer.ready().is_some() && !ask.results().is_empty());
}

fn a_graph_that_fails_or_is_empty_leaves_the_rest() {
    let (harness, _) = open("graph-failed");
    let ask = &shared(&harness).ask;
    assert!(says_hint_of(&harness, &ask.graph));
    assert!(ask.search.ready().is_some());
    let title = (ask.answer.ready())
        .and_then(|answer| answer.title.clone())
        .expect("the answer came back with its title");
    assert!(says(&harness, &title), "the answer is still shown");
    assert!(has(&harness, Role::Button, "Result 1"));

    let (harness, _) = open("no-concepts");
    let graph = shared(&harness)
        .ask
        .graph
        .ready()
        .expect("the graph came back");
    assert!(graph.nodes.is_empty());
    assert_no_failure(&harness);
}

fn an_answer_with_nothing_to_say_is_not_a_failure() {
    let (harness, _) = open("no-answer");
    let answer = shared(&harness).ask.answer.ready();
    assert_eq!(answer.map(|answer| answer.blocks.len()), Some(0));
    assert!(has(&harness, Role::Button, "Result 1"));
    assert_no_failure(&harness);
    assert_nothing_is_waiting(&harness);
}

fn an_ask_for_results_only_writes_no_answer() {
    let (harness, _) = open("results-only");
    assert_eq!(shared(&harness).ask.answer, Loadable::Idle);
    assert!(has(&harness, Role::Button, "Result 1"));
    assert_nothing_is_waiting(&harness);
}

#[test]
fn a_service_that_is_down_is_named_with_what_to_do() {
    a_failed_search_says_why_and_asking_again_needs_no_other_step();
    a_failed_answer_leaves_the_results();
    stores_that_are_down_leave_the_page_that_is_on_disk();
    a_first_run_and_an_empty_library_are_not_failures();
    a_search_that_finds_nothing_says_which_kind_of_nothing();
    a_page_that_is_gone_names_what_to_do_and_leaves_the_answer();
    a_graph_that_fails_or_is_empty_leaves_the_rest();
    an_answer_with_nothing_to_say_is_not_a_failure();
    an_ask_for_results_only_writes_no_answer();
}
